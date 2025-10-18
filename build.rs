use rustflags::Flag;

/// Ensure that the target arch is supported.
///
/// Doing this here instead of emitting `compile_error!` in the lib itself leads to better error
/// messages.
fn check_supported_archs() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    if !["x86_64", "x86", "aarch64", "arm"].contains(&arch.as_str()) {
        println!("cargo::error=closure-ffi does not support the '{arch}' target architecture.");
    }

    // on non-Windows x86, the crate will not function (segfault) without the safe_jit feature.
    // don't allow building without it.
    if arch == "x86"
        && std::env::var("CARGO_CFG_WINDOWS").is_err()
        && std::env::var("CARGO_FEATURE_SAFE_JIT").is_err()
    {
        println!(
            "cargo::error=closure-ffi requires the 'safe_jit' feature to be enabled \
            on non-Windows x86 targets. It will produce incorrect code without it."
        );
    }
}

/// Set a `thumb_mode` cfg on arm targets using thumb encoding by default.
///
/// We need this build script to handle Thumb mode for ARM on a stable
/// release channel, where `target_feature = "thumb_mode"` isn't available.
fn set_thumb_mode_cfg() {
    println!("cargo::rustc-check-cfg=cfg(thumb_mode)");

    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    if arch != "arm" {
        return;
    }

    // Detect thumb mode either from target features (if on Nightly)
    // or by parsing the TARGET env var.
    if std::env::var("CARGO_CFG_TARGET_FEATURE")
        .map(|f| f.contains("thumb-mode"))
        .unwrap_or_else(|_| {
            let target = std::env::var("TARGET").unwrap();
            // In this case, `target-features` can be specified in the custom target spec,
            // so fall back on assuming the feature simply isn't set
            if target.to_lowercase().ends_with(".json") {
                println!(
                    "cargo::warning=Custom ARM target file detected. If using the Thumb ISA by \
                    default, use nightly and add 'thumb-mode' to the feature list."
                );
                false
            }
            else {
                target.starts_with("thumb")
            }
        })
    {
        println!("cargo::rustc-cfg=thumb_mode")
    }
}

// Make sure that the unstable feature is enabled when `-C instrument-coverage` is passed to rustc.
// We need it to add #[coverage(off)] on compiler generated thunks
fn check_coverage_supported() {
    if std::env::var("CARGO_FEATURE_COVERAGE").is_ok() {
        return;
    }

    if rustflags::from_env()
        .any(|f| matches!(f, Flag::Codegen { opt, .. } if opt == "instrument-coverage"))
    {
        println!(
            "cargo::error=closure-ffi requires a nightly compiler and the 'coverage' crate feature \
            to be enabled for '-C instrument-coverage' support."
        );
    }
}

fn main() {
    check_supported_archs();
    check_coverage_supported();
    set_thumb_mode_cfg();
}
