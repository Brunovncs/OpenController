//! macOS. A program cannot create game controllers there without an entitlement Apple grants
//! case by case, and games already read PlayStation, Xbox and Switch Pro controllers directly
//! through the Game Controller framework. So every controller stays as it is: OpenController
//! types keys for their extra buttons, sets their light and shows their battery. The engine's
//! virtual-controller and hiding parts get stand-ins that report this.

use crate::mapping::XusbReport;
use crossbeam_channel::Sender;
use std::path::Path;
use std::sync::Arc;
use std::thread::JoinHandle;

#[derive(Debug)]
pub enum BusError {
    Unsupported,
}

impl std::fmt::Display for BusError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("macOS does not let programs create game controllers")
    }
}

impl BusError {
    pub fn driver(&self) -> crate::engine::Driver {
        crate::engine::Driver::Unsupported
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Feedback {
    pub serial: u32,
    pub large_motor: u8,
    pub small_motor: u8,
}

#[derive(Default)]
pub struct Io;

impl Io {
    pub fn new() -> Io {
        Io
    }
}

pub struct Bus {
    pub path: String,
}

impl Bus {
    pub fn connect() -> Result<Bus, BusError> {
        Err(BusError::Unsupported)
    }

    pub fn plug_x360(&self, _: &mut Io) -> Result<u32, BusError> {
        Err(BusError::Unsupported)
    }

    pub fn unplug(&self, _: &mut Io, _: u32) -> Result<(), u32> {
        Ok(())
    }

    pub fn submit(&self, _: &mut Io, _: u32, _: &XusbReport) -> Result<(), u32> {
        Ok(())
    }

    pub fn listen(self: &Arc<Bus>, _: u32, _: Sender<Feedback>) -> JoinHandle<()> {
        std::thread::spawn(|| {})
    }

    pub fn cancel_all(&self) {}
}

pub const JOURNAL_FILE: &str = "hidden.json";

#[derive(Debug)]
pub enum CloakError {
    NotInstalled,
}

impl std::fmt::Display for CloakError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("not available on macOS")
    }
}

pub struct Cloak;

impl Cloak {
    pub fn start(_: &Path) -> Result<Cloak, CloakError> {
        Err(CloakError::NotInstalled)
    }

    pub fn is_hidden(&self, _: &str) -> bool {
        false
    }

    pub fn hide(&mut self, _: &[String]) -> Result<(), CloakError> {
        Ok(())
    }

    pub fn follow_up(&mut self) -> Option<std::time::Duration> {
        None
    }

    pub fn reveal(&mut self, _: &[String]) -> Result<(), CloakError> {
        Ok(())
    }

    pub fn reveal_all(&mut self) -> Result<(), CloakError> {
        Ok(())
    }
}

pub mod devnode {
    pub fn instance_id(_: &str) -> Option<String> {
        None
    }

    pub fn is_virtual(_: &str) -> bool {
        false
    }

    pub fn is_bluetooth(_: &str) -> bool {
        false
    }

    pub fn is_native(_: &str, _: bool) -> bool {
        true
    }

    pub fn driver_version(_: &str) -> Option<String> {
        None
    }

    pub fn usb_product_name(_: u16, _: u16) -> Option<String> {
        None
    }

    pub fn ancestry(_: &str) -> Vec<String> {
        Vec::new()
    }
}

pub mod bluetooth {
    pub fn disconnect(_: u64) -> bool {
        false
    }
}

/// The Accessibility permission, which typing keys for other programs needs.
pub mod accessibility {
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }

    pub fn granted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    /// Opens System Settings at Privacy & Security, Accessibility.
    pub fn open_settings() {
        let _ =
            std::process::Command::new("open").arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility").spawn();
    }
}

/// The main thread's run loop. AppKit keeps what the workspace knows, such as the application
/// in front, up to date from there, so a process that never turns it reads the state it had at
/// start.
pub mod run_loop {
    use std::ffi::c_void;
    use std::time::Duration;

    #[repr(C)]
    struct TimerContext {
        version: isize,
        info: *mut c_void,
        retain: *const c_void,
        release: *const c_void,
        copy_description: *const c_void,
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRunLoopGetMain() -> *mut c_void;
        fn CFRunLoopRunInMode(mode: *const c_void, seconds: f64, return_after_source_handled: u8) -> i32;
        fn CFRunLoopStop(rl: *mut c_void);
        fn CFAbsoluteTimeGetCurrent() -> f64;
        fn CFRunLoopTimerCreate(
            allocator: *const c_void,
            fire_date: f64,
            interval: f64,
            flags: usize,
            order: isize,
            callout: extern "C" fn(*mut c_void, *mut c_void),
            context: *mut TimerContext,
        ) -> *mut c_void;
        fn CFRunLoopAddTimer(rl: *mut c_void, timer: *mut c_void, mode: *const c_void);
        #[allow(non_upper_case_globals)]
        static kCFRunLoopDefaultMode: *const c_void;
        #[allow(non_upper_case_globals)]
        static kCFRunLoopCommonModes: *const c_void;
    }

    /// `kCFRunLoopRunFinished`: the loop has nothing to wait for and returns at once.
    const FINISHED: i32 = 1;

    /// Turns the main run loop for up to `time`, or until `stop`. Called on the main thread.
    pub fn run_for(time: Duration) {
        let r = unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, time.as_secs_f64(), 0) };
        if r == FINISHED {
            std::thread::sleep(time);
        }
    }

    /// Makes the main thread's `run_for` return now. Callable from any thread; a stop that comes
    /// while the loop is not turning is lost, so the caller checks for work before each turn.
    pub fn stop() {
        unsafe { CFRunLoopStop(CFRunLoopGetMain()) }
    }

    extern "C" fn fire(_: *mut c_void, info: *mut c_void) {
        let f: fn() = unsafe { std::mem::transmute::<*mut c_void, fn()>(info) };
        f();
    }

    /// Calls `f` on the main thread every `interval`, while its run loop turns, for good.
    pub fn every(interval: Duration, f: fn()) {
        let mut context = TimerContext {
            version: 0,
            info: f as *mut c_void,
            retain: std::ptr::null(),
            release: std::ptr::null(),
            copy_description: std::ptr::null(),
        };
        let secs = interval.as_secs_f64();
        unsafe {
            let timer = CFRunLoopTimerCreate(std::ptr::null(), CFAbsoluteTimeGetCurrent() + secs, secs, 0, 0, fire, &mut context);
            if !timer.is_null() {
                // The run loop keeps its own reference, and the timer is never removed.
                CFRunLoopAddTimer(CFRunLoopGetMain(), timer, kCFRunLoopCommonModes);
            }
        }
    }
}

/// The program in front and the programs running, from the workspace.
pub mod workspace {
    use crate::profile::program_name;
    use objc2_app_kit::{NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace};

    fn name(app: &NSRunningApplication) -> Option<String> {
        let url = app.executableURL()?;
        let file = url.lastPathComponent()?;
        Some(program_name(&file.to_string()))
    }

    pub fn active_program() -> Option<String> {
        let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        name(&app)
    }

    /// Programs with a Dock icon, sorted.
    pub fn open_programs() -> Vec<String> {
        let apps = NSWorkspace::sharedWorkspace().runningApplications();
        let mut names: Vec<String> =
            apps.iter().filter(|a| a.activationPolicy() == NSApplicationActivationPolicy::Regular).filter_map(|a| name(&a)).collect();
        names.sort();
        names.dedup();
        names
    }
}
