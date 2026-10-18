//! The application in front, asked of the workspace twice a second on the main thread, whose run
//! loop (turned in `unix.rs`) is where the workspace learns of the change. A notification would
//! need an Objective-C observer for a result no faster than anyone switches programs.

use open_controller_core::macos::{run_loop, workspace::active_program};
use std::sync::Mutex;
use std::time::Duration;

static LAST: Mutex<String> = Mutex::new(String::new());

fn check() {
    let Some(p) = active_program() else { return };
    let Ok(mut last) = LAST.lock() else { return };
    if *last != p {
        last.clone_from(&p);
        drop(last);
        super::report(p);
    }
}

pub fn watch() {
    check();
    run_loop::every(Duration::from_millis(500), check);
}
