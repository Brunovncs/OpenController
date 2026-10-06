//! The application in front, asked of the workspace twice a second: a notification would need
//! an Objective-C observer and a run loop for a result no faster than anyone switches programs.

use open_controller_core::macos::workspace::active_program;
use std::time::Duration;

pub fn watch() {
    std::thread::Builder::new()
        .name("open-controller-foreground".into())
        .spawn(|| {
            let mut last = String::new();
            loop {
                if let Some(p) = active_program()
                    && p != last
                {
                    last = p.clone();
                    super::report(p);
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        })
        .ok();
}
