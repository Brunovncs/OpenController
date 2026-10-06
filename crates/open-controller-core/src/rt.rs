//! Keeping the input thread on time: a deadline-paced poll loop and, on Windows, the "Games"
//! multimedia class for the thread and an opt-out from Windows 11's power throttling (EcoQoS),
//! which otherwise slows a program down once its window is hidden, as it is while you play.

#[cfg(windows)]
pub use self::windows::{Boost, boost_current_thread};
use std::time::{Duration, Instant};

#[cfg(not(windows))]
pub struct Boost;

/// Elsewhere the scheduler gives a busy thread its share without being asked.
#[cfg(not(windows))]
pub fn boost_current_thread() -> Boost {
    Boost
}

#[cfg(windows)]
mod windows {
    use crate::win::wide;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::Threading::{
        AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, GetCurrentProcess, GetCurrentThread,
        PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION, PROCESS_POWER_THROTTLING_STATE, ProcessPowerThrottling, SetProcessInformation,
        SetThreadInformation, SetThreadPriority, THREAD_POWER_THROTTLING_CURRENT_VERSION, THREAD_POWER_THROTTLING_EXECUTION_SPEED,
        THREAD_POWER_THROTTLING_STATE, THREAD_PRIORITY_HIGHEST, ThreadPowerThrottling,
    };

    /// Reverts the thread's multimedia class when dropped.
    pub struct Boost(HANDLE);

    impl Drop for Boost {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { AvRevertMmThreadCharacteristics(self.0) };
            }
        }
    }

    /// Raises the calling thread for low-latency work and keeps the process out of EcoQoS.
    pub fn boost_current_thread() -> Boost {
        unsafe {
            let thread = THREAD_POWER_THROTTLING_STATE {
                Version: THREAD_POWER_THROTTLING_CURRENT_VERSION,
                ControlMask: THREAD_POWER_THROTTLING_EXECUTION_SPEED,
                StateMask: 0,
            };
            SetThreadInformation(
                GetCurrentThread(),
                ThreadPowerThrottling,
                (&thread as *const THREAD_POWER_THROTTLING_STATE).cast(),
                size_of_val(&thread) as u32,
            );
            let process = PROCESS_POWER_THROTTLING_STATE {
                Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
                ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED | PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
                StateMask: 0,
            };
            SetProcessInformation(
                GetCurrentProcess(),
                ProcessPowerThrottling,
                (&process as *const PROCESS_POWER_THROTTLING_STATE).cast(),
                size_of_val(&process) as u32,
            );
            let mut task = 0u32;
            let mmcss = AvSetMmThreadCharacteristicsW(wide("Games").as_ptr(), &mut task);
            if mmcss.is_null() {
                SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
            }
            Boost(mmcss)
        }
    }
}

/// Keeps a loop on a fixed schedule. Windows' high-resolution timer, which `std::thread::sleep`
/// uses, wakes on 0.5 ms boundaries, so asking for 1 ms gives 1.5 ms. Sleeping to an absolute
/// deadline minus a little slack lands on the boundary just before it instead, and the loop
/// runs at the requested rate on average.
pub struct Pacer {
    next: Instant,
}

#[cfg(windows)]
const SLACK: Duration = Duration::from_micros(400);
#[cfg(not(windows))]
const SLACK: Duration = Duration::ZERO;

impl Pacer {
    pub fn new() -> Pacer {
        Pacer { next: Instant::now() }
    }

    pub fn wait(&mut self, period: Duration) {
        self.next += period;
        let now = Instant::now();
        if self.next <= now {
            // Behind schedule (the work took longer, or the machine slept): start over from
            // now rather than running a burst of iterations to catch up.
            self.next = now;
            return;
        }
        std::thread::sleep((self.next - now).saturating_sub(SLACK));
    }
}

impl Default for Pacer {
    fn default() -> Self {
        Self::new()
    }
}
