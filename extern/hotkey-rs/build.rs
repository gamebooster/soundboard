fn main() {
    #[cfg(target_os = "macos")]
    if std::env::var("TARGET").unwrap().contains("-apple") {
        println!("cargo:rustc-link-lib=framework=Carbon");
        cc::Build::new()
            .file("src/carbon_hotkey_binding.c")
            .compile("carbon_hotkey_binding.a");
    }
}
