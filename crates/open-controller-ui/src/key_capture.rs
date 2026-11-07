//! Temporary exclusive keyboard capture for the explicit shortcut picker on Windows.
//! No keys are replayed or logged. The hook exists only for the picker and drains the keys it
//! consumed before ending, so a captured Ctrl/Alt release cannot reach another application.
use open_controller_core::binding::{Chord, modifier};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Holding(u8),
    Chosen(Chord),
    Cancelled,
}

struct Outcome {
    consume: bool,
    event: Option<Event>,
}

struct Recorder {
    inherited: [bool; 256],
    held: [bool; 256],
    chosen: Option<Chord>,
    held_order: Vec<u16>,
    cancelled: bool,
    finished: bool,
}

fn modifier_key(key: u8) -> bool {
    matches!(key, 0x10..=0x12 | 0x5B | 0x5C | 0xA0..=0xA5)
}

impl Recorder {
    fn new(inherited: impl IntoIterator<Item = u8>) -> Self {
        let mut r =
            Self { inherited: [false; 256], held: [false; 256], chosen: None, held_order: Vec::new(), cancelled: false, finished: false };
        for key in inherited {
            r.inherited[key as usize] = true;
        }
        r
    }

    fn mods(&self) -> u8 {
        let any = |keys: &[usize]| keys.iter().any(|&k| self.held[k]);
        (if any(&[0x10, 0xA0, 0xA1]) { modifier::SHIFT } else { 0 })
            | (if any(&[0x11, 0xA2, 0xA3]) { modifier::CTRL } else { 0 })
            | (if any(&[0x12, 0xA4, 0xA5]) { modifier::ALT } else { 0 })
            | (if any(&[0x5B, 0x5C]) { modifier::WIN } else { 0 })
    }

    fn cancel(&mut self) -> Option<Event> {
        self.cancelled = true;
        self.chosen = None;
        self.finish()
    }

    fn finish(&mut self) -> Option<Event> {
        if !self.finished && !self.held.iter().any(|&h| h) && (self.cancelled || self.chosen.is_some()) {
            self.finished = true;
            Some(self.chosen.map_or(Event::Cancelled, Event::Chosen))
        } else {
            None
        }
    }

    fn input(&mut self, key: u8, down: bool) -> Outcome {
        if self.finished {
            return Outcome { consume: false, event: None };
        }
        // Key-downs already delivered before capture must have their key-ups delivered too.
        // Wait for those keys to be released before recording a new chord.
        if self.inherited.iter().any(|&h| h) {
            self.inherited[key as usize] = down;
            return Outcome { consume: false, event: None };
        }
        let was = self.held[key as usize];
        let consume = down || was;
        if was == down {
            return Outcome { consume, event: None };
        }
        self.held[key as usize] = down;
        if !modifier_key(key) {
            if down {
                self.held_order.push(key.into());
            } else {
                self.held_order.retain(|&k| k != u16::from(key));
            }
        }
        if down && !self.cancelled {
            let keys = &self.held_order;
            if keys.as_slice() == [0x1B] && self.mods() == 0 && self.chosen.is_none() {
                self.cancelled = true;
            } else if !keys.is_empty() {
                // Remember the largest set held together, rather than treating sequential
                // taps as a multi-key chord. Modifier order and auto-repeat do not matter.
                let previous = self.chosen.map_or(0, |c| c.keys().count());
                if keys.len() >= previous {
                    self.chosen = Some(Chord::from_keys(self.mods(), keys.iter().copied()));
                }
            }
        }
        Outcome { consume, event: self.finish().or_else(|| Some(Event::Holding(self.mods()))) }
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use async_channel::{Receiver, Sender};
    use std::cell::RefCell;
    use std::ptr::{null, null_mut};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    };
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{GetLastError, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    static ACTIVE: AtomicBool = AtomicBool::new(false);
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    struct Local {
        recorder: Recorder,
        events: Sender<Event>,
        stop: Arc<AtomicBool>,
    }
    thread_local! { static LOCAL: RefCell<Option<Local>> = const { RefCell::new(None) }; }

    struct Active;
    impl Drop for Active {
        fn drop(&mut self) {
            ACTIVE.store(false, Ordering::Release);
        }
    }

    pub struct Capture {
        pub id: u64,
        pub events: Receiver<Event>,
        stop: Arc<AtomicBool>,
    }

    impl Drop for Capture {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
        }
    }

    fn foreground_is_ours() -> bool {
        unsafe {
            let mut pid = 0;
            GetWindowThreadProcessId(GetForegroundWindow(), &mut pid);
            pid == GetCurrentProcessId()
        }
    }

    unsafe extern "system" fn hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
        if code < 0 {
            return unsafe { CallNextHookEx(null_mut(), code, message, data) };
        }
        let down = matches!(message as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
        if !down && !matches!(message as u32, WM_KEYUP | WM_SYSKEYUP) {
            return unsafe { CallNextHookEx(null_mut(), code, message, data) };
        }
        let input = unsafe { &*(data as *const KBDLLHOOKSTRUCT) };
        if input.vkCode > 255 {
            return unsafe { CallNextHookEx(null_mut(), code, message, data) };
        }
        // Normalise the generic modifier VKeys to their physical left/right versions.
        let key = match input.vkCode {
            0x10 => {
                if input.scanCode == 0x36 {
                    0xA1
                } else {
                    0xA0
                }
            }
            0x11 => {
                if input.flags & LLKHF_EXTENDED != 0 {
                    0xA3
                } else {
                    0xA2
                }
            }
            0x12 => {
                if input.flags & LLKHF_EXTENDED != 0 {
                    0xA5
                } else {
                    0xA4
                }
            }
            key => key as u8,
        };
        let consume = LOCAL.with_borrow_mut(|local| {
            let Some(local) = local else { return false };
            if (local.stop.load(Ordering::Acquire) || !foreground_is_ours())
                && let Some(event) = local.recorder.cancel()
            {
                let _ = local.events.try_send(event);
            }
            let out = local.recorder.input(key, down);
            if let Some(event) = out.event {
                let _ = local.events.try_send(event);
            }
            out.consume
        });
        if consume { 1 } else { unsafe { CallNextHookEx(null_mut(), code, message, data) } }
    }

    impl Capture {
        pub fn start() -> Result<Self, String> {
            if ACTIVE.swap(true, Ordering::AcqRel) {
                return Err("Release the previous shortcut keys and try again.".into());
            }
            let (events, rx) = async_channel::unbounded();
            let (ready, started) = mpsc::sync_channel(1);
            let stop = Arc::new(AtomicBool::new(false));
            let cancelled = stop.clone();
            let spawned = std::thread::Builder::new().name("open-controller-key-picker".into()).spawn(move || {
                let _active = Active;
                let inherited = (8..=255)
                    .filter(|&k| !matches!(k, 0x10..=0x12))
                    .filter(|&k| unsafe { GetAsyncKeyState(k) } < 0)
                    .map(|k| k as u8)
                    .collect::<Vec<_>>();
                LOCAL.with_borrow_mut(|local| *local = Some(Local { recorder: Recorder::new(inherited), events, stop: cancelled.clone() }));
                let handle = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), GetModuleHandleW(null()), 0) };
                if handle.is_null() {
                    let _ = ready.send(Err(format!("Windows error {}", unsafe { GetLastError() })));
                    LOCAL.with_borrow_mut(|local| *local = None);
                    return;
                }
                let timer = unsafe { SetTimer(null_mut(), 0, 30, None) };
                if timer == 0 {
                    unsafe {
                        UnhookWindowsHookEx(handle);
                    }
                    let _ = ready.send(Err("Could not start the capture timer.".into()));
                    LOCAL.with_borrow_mut(|local| *local = None);
                    return;
                }
                let _ = ready.send(Ok(()));
                let deadline = Instant::now() + Duration::from_secs(30);
                let mut drain_deadline = None;
                let mut message: MSG = unsafe { std::mem::zeroed() };
                while unsafe { GetMessageW(&mut message, null_mut(), 0, 0) } > 0 {
                    let now = Instant::now();
                    let finished = LOCAL.with_borrow_mut(|local| {
                        let Some(local) = local else { return true };
                        if cancelled.load(Ordering::Acquire) || now >= deadline || !foreground_is_ours() {
                            if let Some(event) = local.recorder.cancel() {
                                let _ = local.events.try_send(event);
                            }
                            drain_deadline.get_or_insert(now + Duration::from_secs(1));
                        }
                        local.recorder.finished
                    });
                    if finished || drain_deadline.is_some_and(|end| now >= end) {
                        break;
                    }
                }
                unsafe {
                    KillTimer(null_mut(), timer);
                    UnhookWindowsHookEx(handle);
                }
                LOCAL.with_borrow_mut(|local| {
                    if let Some(local) = local.take() {
                        let _ = local.events.try_send(Event::Cancelled);
                    }
                });
            });
            if let Err(error) = spawned {
                ACTIVE.store(false, Ordering::Release);
                return Err(error.to_string());
            }
            match started.recv_timeout(Duration::from_secs(2)) {
                Ok(Ok(())) => Ok(Self { id: NEXT_ID.fetch_add(1, Ordering::Relaxed), events: rx, stop }),
                Ok(Err(error)) => {
                    stop.store(true, Ordering::Release);
                    Err(error)
                }
                Err(error) => {
                    stop.store(true, Ordering::Release);
                    Err(error.to_string())
                }
            }
        }
    }
}

#[cfg(windows)]
pub use windows::Capture;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ctrl_v_up_records_both_keys_and_waits_for_every_release() {
        let expected = Chord::from_keys(modifier::CTRL, [0x56, 0x26]);
        for releases in [[0x26, 0x56, 0xA2], [0xA2, 0x56, 0x26]] {
            let mut r = Recorder::new([]);
            for key in [0xA2, 0x56, 0x26] {
                assert!(r.input(key, true).consume);
            }
            assert_eq!(r.chosen, Some(expected));
            assert_eq!(r.input(0x26, true).event, None);
            for key in &releases[..2] {
                assert!(!matches!(r.input(*key, false).event, Some(Event::Chosen(_))));
            }
            assert_eq!(r.input(releases[2], false).event, Some(Event::Chosen(expected)));
        }
    }

    #[test]
    fn sequential_taps_are_not_combined_and_late_modifiers_are_captured() {
        let mut r = Recorder::new([]);
        r.input(0xA2, true);
        r.input(0x56, true);
        r.input(0x56, false);
        r.input(0x26, true);
        r.input(0x26, false);
        assert_eq!(r.input(0xA2, false).event, Some(Event::Chosen(Chord::new(modifier::CTRL, 0x26))));
        let mut r = Recorder::new([]);
        r.input(0x56, true);
        r.input(0xA2, true);
        r.input(0x26, true);
        assert_eq!(r.chosen, Some(Chord::from_keys(modifier::CTRL, [0x56, 0x26])));
    }

    #[test]
    fn shortcut_is_consumed_and_confirmed_only_after_every_key_is_released() {
        let mut r = Recorder::new([]);
        for key in [0xA2, 0xA4, 0x79] {
            assert!(r.input(key, true).consume);
        }
        assert_eq!(r.chosen, Some(Chord::new(modifier::CTRL | modifier::ALT, 0x79)));
        assert_eq!(r.input(0x79, true).event, None, "repeat does not choose twice");
        for key in [0x79, 0xA4] {
            let out = r.input(key, false);
            assert!(out.consume);
            assert!(!matches!(out.event, Some(Event::Chosen(_))));
        }
        let out = r.input(0xA2, false);
        assert!(out.consume);
        assert_eq!(out.event, Some(Event::Chosen(Chord::new(5, 0x79))));
        assert!(!r.input(0x41, true).consume, "normal typing resumes after capture");
    }
    #[test]
    fn cancellation_drains_captured_modifiers_without_assigning_anything() {
        let mut r = Recorder::new([]);
        r.input(0xA2, true);
        assert_eq!(r.cancel(), None);
        assert!(r.input(0x79, true).consume);
        r.input(0x79, false);
        assert_eq!(r.input(0xA2, false).event, Some(Event::Cancelled));
        assert_eq!(r.chosen, None);
    }
    #[test]
    fn keys_already_down_keep_their_release_and_do_not_become_a_shortcut() {
        let mut r = Recorder::new([0xA2]);
        assert!(!r.input(0xA2, false).consume);
        assert!(r.input(0x79, true).consume);
        assert_eq!(r.input(0x79, false).event, Some(Event::Chosen(Chord::new(0, 0x79))));
    }
    #[test]
    fn escape_cancels_and_unmatched_releases_pass_through() {
        let mut r = Recorder::new([]);
        assert!(!r.input(0x41, false).consume);
        assert!(r.input(0x1B, true).consume);
        assert_eq!(r.input(0x1B, false).event, Some(Event::Cancelled));
    }
    #[test]
    fn modified_escape_is_recorded_including_task_manager_shortcut() {
        for modifiers in [vec![0xA2, 0xA0], vec![0xA2], vec![0xA4]] {
            let mut r = Recorder::new([]);
            for &key in &modifiers {
                assert!(r.input(key, true).consume);
            }
            let expected = Chord::new(r.mods(), 0x1B);
            assert!(r.input(0x1B, true).consume);
            assert_eq!(r.chosen, Some(expected));
            assert!(!r.cancelled);
            assert!(r.input(0x1B, false).consume);
            let mut last = None;
            for key in modifiers {
                last = r.input(key, false).event;
            }
            assert_eq!(last, Some(Event::Chosen(expected)));
        }
    }

    #[test]
    fn releasing_only_modifiers_keeps_the_picker_open() {
        let mut r = Recorder::new([]);
        r.input(0xA2, true);
        assert_eq!(r.input(0xA2, false).event, Some(Event::Holding(0)));
        assert!(!r.finished);
        assert_eq!(r.cancel(), Some(Event::Cancelled));
    }
}
