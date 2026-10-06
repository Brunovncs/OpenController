//! The Windows device tree: from a device interface path (what SDL reports) to the device
//! instance behind it (what HidHide lists), and up to its ancestors.

use crate::win::{from_multi_sz, from_wide, wide};
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    CM_GETIDLIST_FILTER_ENUMERATOR, CM_GETIDLIST_FILTER_PRESENT, CM_Get_DevNode_PropertyW, CM_Get_Device_ID_List_SizeW,
    CM_Get_Device_ID_ListW, CM_Get_Device_IDW, CM_Get_Device_Interface_PropertyW, CM_Get_Parent, CM_LOCATE_DEVNODE_NORMAL,
    CM_Locate_DevNodeW, CR_SUCCESS,
};
use windows_sys::Win32::Devices::Properties::{
    DEVPKEY_Device_BusReportedDeviceDesc, DEVPKEY_Device_DriverVersion, DEVPKEY_Device_InstanceId, DEVPKEY_Device_Service,
};
use windows_sys::Win32::Foundation::DEVPROPKEY;

const MAX_DEPTH: usize = 12;

/// The device instance id (`HID\VID_054C&PID_09CC&MI_03\7&2B5C3C1A&0&0000`) behind an interface
/// path (`\\?\HID#VID_054C&PID_09CC&MI_03#7&2b5c3c1a&0&0000#{...}`).
pub fn instance_id(interface_path: &str) -> Option<String> {
    let mut buf = [0u16; 512];
    let mut size = size_of_val(&buf) as u32;
    let mut ty = 0u32;
    let ok = unsafe {
        CM_Get_Device_Interface_PropertyW(
            wide(interface_path).as_ptr(),
            &DEVPKEY_Device_InstanceId,
            &mut ty,
            buf.as_mut_ptr().cast(),
            &mut size,
            0,
        )
    };
    (ok == CR_SUCCESS).then(|| from_wide(&buf)).filter(|s| !s.is_empty())
}

fn locate(instance_id: &str) -> Option<u32> {
    let mut node = 0u32;
    let ok = unsafe { CM_Locate_DevNodeW(&mut node, wide(instance_id).as_ptr(), CM_LOCATE_DEVNODE_NORMAL) };
    (ok == CR_SUCCESS).then_some(node)
}

fn parent(node: u32) -> Option<u32> {
    let mut p = 0u32;
    (unsafe { CM_Get_Parent(&mut p, node, 0) } == CR_SUCCESS).then_some(p)
}

fn id_of(node: u32) -> Option<String> {
    let mut buf = [0u16; 512];
    (unsafe { CM_Get_Device_IDW(node, buf.as_mut_ptr(), buf.len() as u32, 0) } == CR_SUCCESS).then(|| from_wide(&buf))
}

fn string_property(node: u32, key: &DEVPROPKEY) -> Option<String> {
    let mut buf = [0u16; 256];
    let mut size = size_of_val(&buf) as u32;
    let mut ty = 0u32;
    let ok = unsafe { CM_Get_DevNode_PropertyW(node, key, &mut ty, buf.as_mut_ptr().cast(), &mut size, 0) };
    (ok == CR_SUCCESS).then(|| from_wide(&buf))
}

/// The device and its ancestors, nearest first, as (instance id, service).
fn lineage(instance_id: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut node = locate(instance_id);
    while let Some(n) = node {
        if out.len() == MAX_DEPTH {
            break;
        }
        out.push((id_of(n).unwrap_or_default(), string_property(n, &DEVPKEY_Device_Service).unwrap_or_default()));
        node = parent(n);
    }
    out
}

/// True when the device descends from ViGEmBus: one of the virtual controllers this or another
/// program plugged in. Reading those back would loop input into itself.
pub fn is_virtual(interface_path: &str) -> bool {
    instance_id(interface_path).is_some_and(|id| lineage(&id).iter().any(|(_, service)| service.eq_ignore_ascii_case("ViGEmBus")))
}

/// The version of the driver behind a device interface, as Device Manager shows it.
pub fn driver_version(interface_path: &str) -> Option<String> {
    let node = locate(&instance_id(interface_path)?)?;
    string_property(node, &DEVPKEY_Device_DriverVersion).filter(|v| !v.is_empty())
}

/// Instance ids of the present devices an enumerator (`USB`, `HID`, ...) created.
fn present_ids(enumerator: &str) -> Vec<String> {
    let filter = wide(enumerator);
    let flags = CM_GETIDLIST_FILTER_ENUMERATOR | CM_GETIDLIST_FILTER_PRESENT;
    let mut len = 0u32;
    if unsafe { CM_Get_Device_ID_List_SizeW(&mut len, filter.as_ptr(), flags) } != CR_SUCCESS {
        return Vec::new();
    }
    let mut buf = vec![0u16; len as usize];
    if unsafe { CM_Get_Device_ID_ListW(filter.as_ptr(), buf.as_mut_ptr(), len, flags) } != CR_SUCCESS {
        return Vec::new();
    }
    from_multi_sz(&buf)
}

/// The name a USB controller gives itself (its product string, as Device Manager shows it under
/// "Bus reported device description"), found by vendor and product id. For controllers SDL reads
/// through XInput, which reports no name: an 8BitDo receiver is then "8BitDo Ultimate 2 Wireless
/// Controller for PC" instead of "XInput Controller". Virtual controllers are skipped.
pub fn usb_product_name(vendor: u16, product: u16) -> Option<String> {
    let prefix = format!(r"USB\VID_{vendor:04X}&PID_{product:04X}");
    present_ids("USB")
        .into_iter()
        .filter(|id| id.to_ascii_uppercase().starts_with(&prefix))
        .filter(|id| !lineage(id).iter().any(|(_, service)| service.eq_ignore_ascii_case("ViGEmBus")))
        .find_map(|id| string_property(locate(&id)?, &DEVPKEY_Device_BusReportedDeviceDesc))
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
}
