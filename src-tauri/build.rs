fn main() {
    #[cfg(target_os = "macos")]
    swift_rs::SwiftLinker::new("14.0")
        .with_package("AxPlugin", "./plugins/ax/macos")
        .link();

    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

    tauri_build::build()
}
