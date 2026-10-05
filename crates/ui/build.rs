// The icon is resource 1, which GPUI uses for the window. No manifest here: GPUI embeds its own.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=../../assets/ui.rc");
        println!("cargo:rerun-if-changed=../../assets/icon.ico");
        embed_resource::compile("../../assets/ui.rc", embed_resource::NONE).manifest_optional().unwrap();
    }
}
