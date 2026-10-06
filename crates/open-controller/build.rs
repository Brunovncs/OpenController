// Resource 1 is the icon, for Explorer; 2 and 3 are the notification-area icon for a dark and a
// light taskbar. The manifest opts into
// Common Controls 6, without which the tray menu's TaskDialogIndirect import fails and the
// program does not start, and into per-monitor DPI awareness.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=../../assets/tray.rc");
        println!("cargo:rerun-if-changed=../../assets/tray.manifest");
        println!("cargo:rerun-if-changed=../../assets/icon.ico");
        println!("cargo:rerun-if-changed=../../assets/tray-dark.ico");
        println!("cargo:rerun-if-changed=../../assets/tray-light.ico");
        embed_resource::compile("../../assets/tray.rc", embed_resource::NONE).manifest_optional().unwrap();
    }
}
