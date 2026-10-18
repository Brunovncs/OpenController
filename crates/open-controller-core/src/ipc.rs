//! The link between the resident process (engine and tray icon) and the window, which runs as
//! a separate process only while it is open. Newline-separated JSON messages go both ways over a
//! channel only the user can open: a named pipe local to the session on Windows, a Unix socket
//! in the user's runtime directory elsewhere.

use crate::engine::{PadKey, Snapshot};
use crate::i18n::Lang;
use crate::platform::VirtualDriver;
use crate::profile::Edit;
use crate::report::Diagnosis;
use serde::{Deserialize, Serialize};

#[cfg_attr(windows, path = "ipc/windows.rs")]
#[cfg_attr(unix, path = "ipc/unix.rs")]
mod pipe;

pub use pipe::{Pipe, Reader, is_disconnect, remove_socket};

/// The version of these messages. Raised whenever either side would misread the other's, so a
/// window from one release meeting a resident process from another can tell.
pub const PROTO: u32 = 1;

/// What the resident process tells the window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToWindow {
    Snapshot(Snapshot),
    Prefs(Prefs),
    /// The answer to [`ToTray::Diagnose`]; `None` when the controller is gone.
    Diagnosis(PadKey, Option<Box<Diagnosis>>),
    /// The first message on a connection. Resident processes before it send none.
    Hello {
        proto: u32,
        version: String,
    },
    /// The resident process is quitting on purpose: the window closes with it instead of
    /// starting it again.
    Bye,
}

/// Reads one line as a message. A line that does not parse is skipped, but counted and logged,
/// so a program on the other end that speaks another version does not go unnoticed.
#[cfg_attr(windows, allow(dead_code))]
pub(crate) fn parse<T: serde::de::DeserializeOwned>(line: &[u8], skipped: &mut u64) -> Option<T> {
    match serde_json::from_slice(line) {
        Ok(msg) => Some(msg),
        Err(e) => {
            *skipped += 1;
            // 1, 2, 4, 8...: a stream of them does not flood the log.
            if skipped.is_power_of_two() {
                eprintln!("open-controller: skipped a message that did not parse ({skipped} so far): {e}");
            }
            None
        }
    }
}

/// What the window asks of the resident process.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToTray {
    SetHiding(bool),
    SetAutostart(bool),
    SetCheckUpdates(bool),
    SetLanguage(Lang),
    Identify(PadKey),
    PowerOff(PadKey),
    /// Changes the profiles of the controllers whose settings are kept under `store`.
    Edit {
        store: String,
        edit: Edit,
    },
    /// Two controllers trade players.
    SwapPlayers(PadKey, PadKey),
    /// What is known about a controller, for a report on it.
    Diagnose(PadKey),
    /// The driver that makes the virtual controllers (Windows).
    SetVirtualDriver(VirtualDriver),
    /// Each Joy-Con a controller of its own, or left and right ones joined in pairs.
    SplitJoyCons(bool),
    /// Whether the window is the one in front: behind a game it is sent live input less often.
    Focused(bool),
    Quit,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub hide_originals: bool,
    pub autostart: bool,
    /// Ask GitHub for a newer version when the window opens.
    pub check_updates: bool,
    pub lang: Lang,
    pub virtual_driver: VirtualDriver,
    /// Each Joy-Con a controller of its own, held sideways.
    pub split_joycons: bool,
}

#[cfg(test)]
mod tests {
    use super::pipe::pipe_name;
    use super::*;
    use crate::engine::Snapshot;

    fn prefs() -> Prefs {
        Prefs {
            hide_originals: true,
            autostart: false,
            check_updates: true,
            lang: Lang::Pt,
            virtual_driver: VirtualDriver::Viiper,
            split_joycons: true,
        }
    }

    #[test]
    fn messages_round_trip_over_a_real_pipe() {
        let name = format!("{}.test{}", pipe_name(), std::process::id());
        let server_name = name.clone();
        let server = std::thread::spawn(move || {
            let pipe = Pipe::create_at(&server_name).expect("create");
            pipe.accept().expect("accept");
            pipe.send(&ToWindow::Hello { proto: PROTO, version: "1.2.3".into() }).unwrap();
            pipe.send(&ToWindow::Prefs(prefs())).unwrap();
            // A large message, split across several reads.
            let big = Snapshot { sdl_error: Some("x".repeat(100_000)), ..Snapshot::default() };
            pipe.send(&ToWindow::Snapshot(big)).unwrap();
            let mut r = pipe.reader();
            let got: ToTray = r.recv().unwrap();
            assert_eq!(got, ToTray::Identify(PadKey::Slot(3)));
            let gone = r.recv::<ToTray>().unwrap_err();
            assert!(is_disconnect(&gone), "{gone:?}");
        });
        let client = loop {
            if let Some(p) = Pipe::connect_at(&name).unwrap() {
                break p;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        let mut r = client.reader();
        assert_eq!(r.recv::<ToWindow>().unwrap(), ToWindow::Hello { proto: PROTO, version: "1.2.3".into() });
        assert_eq!(r.recv::<ToWindow>().unwrap(), ToWindow::Prefs(prefs()));
        match r.recv::<ToWindow>().unwrap() {
            ToWindow::Snapshot(s) => assert_eq!(s.sdl_error.map(|e| e.len()), Some(100_000)),
            other => panic!("{other:?}"),
        }
        client.send(&ToTray::Identify(PadKey::Slot(3))).unwrap();
        drop(r);
        drop(client);
        server.join().unwrap();
    }

    #[test]
    fn unknown_messages_are_counted() {
        let mut skipped = 0;
        assert_eq!(parse::<ToWindow>(b"\"Bye\"", &mut skipped), Some(ToWindow::Bye));
        assert_eq!(parse::<ToWindow>(b"{\"Later\":1}", &mut skipped), None);
        assert_eq!(parse::<ToWindow>(b"not json", &mut skipped), None);
        assert_eq!(skipped, 2);
    }
}
