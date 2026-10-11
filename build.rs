// Embeds icon/icon.ico (+ version info) into the exe: Explorer, taskbar and title bar all use it.
fn main() {
    println!("cargo:rerun-if-changed=icon/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icon/icon.ico");
        res.set("FileDescription", "TurboText");
        res.set("ProductName", "TurboText");
        res.compile()
            .expect("failed to embed icon (needs the Windows SDK rc.exe)");
    }
}
