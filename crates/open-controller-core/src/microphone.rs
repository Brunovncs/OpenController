//! Windows' default recording endpoint. Only its mute flag is read; audio is never captured.
//! COM and endpoint discovery stay on a worker, away from the controller's input thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

pub struct Monitor {
    state: Arc<AtomicU8>,
    #[cfg(windows)]
    tx: std::sync::mpsc::SyncSender<()>,
}

impl Monitor {
    pub fn start() -> Self {
        let state = Arc::new(AtomicU8::new(0));
        #[cfg(windows)]
        {
            let (tx, rx) = std::sync::mpsc::sync_channel(8);
            let shared = state.clone();
            let _ = std::thread::Builder::new().name("open-controller-microphone".into()).spawn(move || windows_worker(rx, shared));
            Self { state, tx }
        }
        #[cfg(not(windows))]
        Self { state }
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
        let _ = self.tx.try_send(());
    }
}

#[cfg(windows)]
fn windows_worker(rx: std::sync::mpsc::Receiver<()>, state: Arc<AtomicU8>) {
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Duration;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eConsole};
    use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize};

    unsafe {
        if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
            return;
        }
        let query = |toggle: bool| -> windows::core::Result<bool> {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let endpoint = enumerator.GetDefaultAudioEndpoint(eCapture, eConsole)?;
            let volume: IAudioEndpointVolume = endpoint.Activate(CLSCTX_ALL, None)?;
            let muted = volume.GetMute()?.as_bool();
            if toggle {
                volume.SetMute(!muted, std::ptr::null())?;
            }
            Ok(volume.GetMute()?.as_bool())
        };
        let mut toggle = false;
        loop {
            let value = match query(toggle) {
                Ok(false) => 1,
                Ok(true) => 2,
                Err(_) => 0,
            };
            state.store(value, Ordering::Release);
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(()) => toggle = true,
                Err(RecvTimeoutError::Timeout) => toggle = false,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
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
