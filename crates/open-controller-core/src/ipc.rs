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

/// What the resident process tells the window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToWindow {
    Snapshot(Snapshot),
    Prefs(Prefs),
    /// The answer to [`ToTray::Diagnose`]; `None` when the controller is gone.
    Diagnosis(PadKey, Option<Box<Diagnosis>>),
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
}

#[cfg(test)]
mod tests {
    use super::pipe::pipe_name;
    use super::*;
    use crate::engine::Snapshot;

    fn prefs() -> Prefs {
        Prefs { hide_originals: true, autostart: false, check_updates: true, lang: Lang::Pt, virtual_driver: VirtualDriver::Viiper }
    }

    #[test]
    fn messages_round_trip_over_a_real_pipe() {
        let name = format!("{}.test{}", pipe_name(), std::process::id());
        let server_name = name.clone();
        let server = std::thread::spawn(move || {
            let pipe = Pipe::create_at(&server_name).expect("create");
            pipe.accept().expect("accept");
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
}
