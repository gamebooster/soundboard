extern crate cc;

fn main() {
    fn parse_env(key: &str, default: bool) -> bool {
        use std::env::{var, VarError};

        match var(key) {
            Ok(val) => match &val as &str {
                "0" => false,
                "1" => true,
                _ => default,
            },
            Err(VarError::NotPresent) => default,
            Err(VarError::NotUnicode(_)) => panic!("Environment variable is not unicode: {}", key),
        }
    }

    let linear_interpolation = parse_env("XM_LINEAR_INTERPOLATION", true);
    let ramping = parse_env("XM_RAMPING", true);
    let debug = parse_env("XM_DEBUG", false);
    let big_endian = parse_env("XM_BIG_ENDIAN", false);

    fn on_off(value: bool) -> Option<&'static str> {
        Some(if value { "1" } else { "0" })
    }

    #[cfg(target_os = "windows")]
    std::env::set_var("CC", "clang");

    cc::Build::new()
        .file("libxm/src/context.c")
        .file("libxm/src/load.c")
        .file("libxm/src/play.c")
        .file("libxm/src/xm.c")
        .include("libxm/include")
        .define("XM_LINEAR_INTERPOLATION", on_off(linear_interpolation))
        .define("XM_RAMPING", on_off(ramping))
        .define("XM_DEBUG", on_off(debug))
        .define("XM_BIG_ENDIAN", on_off(big_endian))
        .define("XM_DEFENSIVE", "0")
        .define("XM_LIBXMIZE_DELTA_SAMPLES", "1")
        .compile("libxm.a");
}
