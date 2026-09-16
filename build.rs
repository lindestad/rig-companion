fn main() {
    println!("cargo:rerun-if-changed=assets/rig-companion.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/rig-companion.ico")
            .set("ProductName", "Rig Companion")
            .set("FileDescription", "Rig Companion")
            .compile()
            .expect("embed Windows app icon");
    }
}
