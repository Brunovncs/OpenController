//! Turning a Bluetooth controller off from the computer, by dropping its link. A DualShock 4,
//! DualSense or Switch Pro controller powers down when the host disconnects it.

use crate::win::{Handle, ioctl_sync};
use windows_sys::Win32::Devices::Bluetooth::{
    BLUETOOTH_FIND_RADIO_PARAMS, BluetoothFindFirstRadio, BluetoothFindNextRadio, BluetoothFindRadioClose,
};
use windows_sys::Win32::Foundation::HANDLE;

/// `IOCTL_BTH_DISCONNECT_DEVICE` from bthioctl.h.
const IOCTL_BTH_DISCONNECT_DEVICE: u32 = 0x41000C;

/// Asks every local radio to disconnect the device. Returns true if one accepted.
pub fn disconnect(address: u64) -> bool {
    let params = BLUETOOTH_FIND_RADIO_PARAMS { dwSize: size_of::<BLUETOOTH_FIND_RADIO_PARAMS>() as u32 };
    let mut radio: HANDLE = std::ptr::null_mut();
    let find = unsafe { BluetoothFindFirstRadio(&params, &mut radio) };
    if find.is_null() {
        return false;
    }
    let mut done = false;
    loop {
        if let Some(r) = Handle::valid(radio) {
            done |= ioctl_sync(r.0, IOCTL_BTH_DISCONNECT_DEVICE, &address.to_le_bytes(), &mut []).is_ok();
        }
        if unsafe { BluetoothFindNextRadio(find, &mut radio) } == 0 {
            break;
        }
    }
    unsafe { BluetoothFindRadioClose(find) };
    done
}
