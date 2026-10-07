//! Linux: virtual controllers through uinput, controllers hidden from games with an exclusive
//! evdev grab, keys typed through a uinput keyboard, devices looked up in sysfs, and the program
//! in front read from the X server.

pub mod devnode;
pub mod ff;
pub mod grab;
pub mod setup;
pub mod uinput;
pub mod x11;

/// `_IO`, `_IOW` and `_IOWR` from `<asm-generic/ioctl.h>`.
mod ioc {
    const fn make(dir: u32, ty: u8, nr: u8, size: usize) -> libc::c_ulong {
        ((dir << 30) | ((size as u32) << 16) | ((ty as u32) << 8) | nr as u32) as libc::c_ulong
    }

    pub const fn io(ty: u8, nr: u8) -> libc::c_ulong {
        make(0, ty, nr, 0)
    }

    pub const fn iow<T>(ty: u8, nr: u8) -> libc::c_ulong {
        make(1, ty, nr, size_of::<T>())
    }

    pub const fn iowr<T>(ty: u8, nr: u8) -> libc::c_ulong {
        make(3, ty, nr, size_of::<T>())
    }
}

/// Turning a Bluetooth controller off by dropping its link, through BlueZ.
pub mod bluetooth {
    /// Asks BlueZ to disconnect the device. Returns true if it did.
    pub fn disconnect(address: u64) -> bool {
        let b = address.to_be_bytes();
        let mac = format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", b[2], b[3], b[4], b[5], b[6], b[7]);
        std::process::Command::new("bluetoothctl")
            .args(["disconnect", &mac])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
}
