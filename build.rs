use std::env::var;

/// Ensure that the target arch is supported.
///
/// Doing this here instead of emitting `compile_error!` in the lib itself leads to better error
/// messages.
fn check_supported_archs() {
    let arch = var("CARGO_CFG_TARGET_ARCH").unwrap();

    if !["x86_64", "x86", "aarch64", "arm"].contains(&arch.as_str()) {
        println!("cargo::error=closure-ffi does not support the '{arch}' target architecture.");
    }

    // We don't support legacy ARM (v4/v5).
    if arch == "arm" {
        let target = var("TARGET").unwrap();
        let target_arch = target.split('-').next().unwrap();
        if target_arch.contains("v4") || target_arch.contains("v5") {
            println!(
                "cargo::error=closure-ffi does not support the '{target_arch}' target architecture."
            );
        }
    }
}

fn main() {
    check_supported_archs();

    #[cfg(target_arch = "arm")]
    arm_targets::process();
}
