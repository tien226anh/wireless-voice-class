fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icons/wireless-pa.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/icons/wireless-pa.ico")
            .set("ProductName", "Wireless PA")
            .set("FileDescription", "Wireless PA microphone to speaker")
            .set("InternalName", "wireless-pa")
            .set("OriginalFilename", "wireless-pa.exe")
            .compile()
            .expect("Failed to embed the Windows application icon");
    }
}
