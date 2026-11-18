//! Windows' default recording endpoint. Only its mute flag is read; audio is never captured.
//! COM and endpoint discovery stay on a worker, away from the controller's input thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

pub struct Monitor {
    state: Arc<AtomicU8>,
    wanted: bool,
    #[cfg(windows)]
    tx: std::sync::mpsc::SyncSender<Ask>,
}

#[cfg(windows)]
enum Ask {
    Watch(bool),
    Toggle,
}

impl Monitor {
    pub fn start() -> Self {
        let state = Arc::new(AtomicU8::new(0));
        #[cfg(windows)]
        {
            let (tx, rx) = std::sync::mpsc::sync_channel(8);
            let shared = state.clone();
            let _ = std::thread::Builder::new().name("open-controller-microphone".into()).spawn(move || windows_worker(rx, shared));
            Self { state, wanted: false, tx }
        }
        #[cfg(not(windows))]
        Self { state, wanted: false }
    }

    /// Whether the mute flag is read: only while a controller with a mic button is connected.
    pub fn watch(&mut self, wanted: bool) {
        if self.wanted == wanted {
            return;
        }
        #[cfg(windows)]
        if self.tx.try_send(Ask::Watch(wanted)).is_err() {
            return;
        }
        self.wanted = wanted;
    }

    pub fn muted(&self) -> Option<bool> {
        match self.state.load(Ordering::Acquire) {
            1 => Some(false),
            2 => Some(true),
            _ => None,
        }
    }

    pub fn toggle(&self) {
        #[cfg(windows)]
        let _ = self.tx.try_send(Ask::Toggle);
    }
}

#[cfg(windows)]
fn windows_worker(rx: std::sync::mpsc::Receiver<Ask>, state: Arc<AtomicU8>) {
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Duration;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eConsole};
    use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize};

    unsafe {
        if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
            return;
        }
        let mut enumerator: Option<IMMDeviceEnumerator> = None;
        let mut query = |toggle: bool| -> windows::core::Result<bool> {
            if enumerator.is_none() {
                enumerator = Some(CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?);
            }
            let Some(enumerator) = &enumerator else { unreachable!() };
            // The default device can change: it is looked up each time.
            let endpoint = enumerator.GetDefaultAudioEndpoint(eCapture, eConsole)?;
            let volume: IAudioEndpointVolume = endpoint.Activate(CLSCTX_ALL, None)?;
            let muted = volume.GetMute()?.as_bool();
            if toggle {
                volume.SetMute(!muted, std::ptr::null())?;
            }
            Ok(volume.GetMute()?.as_bool())
        };
        let mut watching = false;
        let mut toggle = false;
        loop {
            if watching {
                let value = match query(toggle) {
                    Ok(false) => 1,
                    Ok(true) => 2,
                    Err(_) => 0,
                };
                state.store(value, Ordering::Release);
            }
            toggle = false;
            // Not watching, it sleeps until asked.
            let ask =
                if watching { rx.recv_timeout(Duration::from_millis(200)) } else { rx.recv().map_err(|_| RecvTimeoutError::Disconnected) };
            match ask {
                Ok(Ask::Watch(on)) => {
                    watching = on;
                    if !on {
                        state.store(0, Ordering::Release);
                    }
                }
                Ok(Ask::Toggle) => toggle = watching,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        drop(query);
        drop(enumerator);
        CoUninitialize();
    }
}

pub fn pressed(before: u64, after: u64) -> bool {
    let mask = 1 << crate::mapping::button::MISC1;
    before & mask == 0 && after & mask != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mic_press_toggles_once_until_released() {
        let mic = 1 << crate::mapping::button::MISC1;
        assert!(pressed(0, mic));
        assert!(!pressed(mic, mic));
        assert!(!pressed(mic, 0));
        assert!(!pressed(0, 1));
        assert!(pressed(0, mic));
    }
}
