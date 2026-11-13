//! Desktop mouse output is isolated from the input loop. A watchdog releases abandoned clicks.
use crate::lab::Mouse;

pub struct Desktop {
    #[cfg(windows)]
    tx: crossbeam_channel::Sender<(u32, Mouse)>,
}
impl Desktop {
    pub fn start() -> Self {
        #[cfg(windows)]
        {
            let (tx, rx) = crossbeam_channel::bounded(256);
            let _ = std::thread::Builder::new().name("open-controller-mouse".into()).spawn(move || worker(rx));
            Self { tx }
        }
        #[cfg(not(windows))]
        Self {}
    }
    pub fn send(&self, id: u32, mouse: Mouse) {
        #[cfg(windows)]
        let _ = self.tx.try_send((id, mouse));
        #[cfg(not(windows))]
        let _ = (id, mouse);
    }
}

#[cfg(windows)]
fn worker(rx: crossbeam_channel::Receiver<(u32, Mouse)>) {
    use std::collections::HashMap;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
        MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput,
    };
    let emit = |dx: i32, dy: i32, data: u32, flags: u32| {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 { mi: MOUSEINPUT { dx, dy, mouseData: data, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
        };
        unsafe { SendInput(1, &input, size_of::<INPUT>() as i32) };
    };
    let mut owners: HashMap<u32, (u8, Instant)> = HashMap::new();
    let mut held = 0u8;
    let mut residual = (0f32, 0f32, 0f32);
    loop {
        let mut tap = false;
        let mut disconnected = false;
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok((id, m)) => {
                owners.insert(id, (m.buttons & 3, Instant::now()));
                residual.0 += m.dx.clamp(-20000., 20000.);
                residual.1 += m.dy.clamp(-20000., 20000.);
                residual.2 += m.wheel.clamp(-2400., 2400.);
                tap = m.tap;
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                owners.clear();
                disconnected = true;
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
        }
        owners.retain(|_, (_, at)| at.elapsed() < Duration::from_millis(250));
        let buttons = owners.values().fold(0, |bits, (b, _)| bits | b);
        for (mask, down, up) in [(1, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP), (2, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP)] {
            if (held ^ buttons) & mask != 0 {
                emit(0, 0, 0, if buttons & mask != 0 { down } else { up });
            }
        }
        held = buttons;
        let (dx, dy, wheel) = (residual.0.trunc() as i32, residual.1.trunc() as i32, residual.2.trunc() as i32);
        residual.0 -= dx as f32;
        residual.1 -= dy as f32;
        residual.2 -= wheel as f32;
        if dx != 0 || dy != 0 {
            emit(dx, dy, 0, MOUSEEVENTF_MOVE);
        }
        if wheel != 0 {
            emit(0, 0, wheel as u32, MOUSEEVENTF_WHEEL);
        }
        if tap && held & 1 == 0 {
            emit(0, 0, 0, MOUSEEVENTF_LEFTDOWN);
            emit(0, 0, 0, MOUSEEVENTF_LEFTUP);
        }
        if disconnected {
            break;
        }
    }
}
