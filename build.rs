use std::env::var;

use rustflags::Flag;

/// Ensure that the target arch is supported.
///
/// Doing this here instead of emitting `compile_error!` in the lib itself leads to better error
/// messages.
fn check_supported_archs() {
    let arch = var("CARGO_CFG_TARGET_ARCH").unwrap();

    if !["x86_64", "x86", "aarch64", "arm"].contains(&arch.as_str()) {
        println!("cargo::error=closure-ffi does not support the '{arch}' target architecture.");
    }
}

/// Emit a warning if safe_jit is turned off, unless
///
/// Also error if safe_jit is not used on a platform where it is mandatory for the crate to
/// function.
fn no_safe_jit_warn() {
    if var("CARGO_FEATURE_SAFE_JIT").is_ok() {
        return;
    }

    // On non-Windows x86, the crate will not function (segfault) without the safe_jit feature.
    // Don't allow building without it.
    if var("CARGO_CFG_TARGET_ARCH").unwrap() == "x86" && var("CARGO_CFG_WINDOWS").is_err() {
        println!(
            "cargo::error=closure-ffi requires the 'safe_jit' feature to be enabled \
            on non-Windows x86 targets. It *will* produce incorrect code without it."
        );
        return;
    }

    // Check if the user has opted into not using safe_jit
    if var("CARGO_FEATURE_NO_SAFE_JIT").is_ok() {
        return;
    }

    // Otherwise, emit an error unless the suppress feature is on
    println!(
        "cargo::error=closure-ffi is being built without the 'safe_jit' feature. \
        This may lead to incorrect code being generated. Consider enabling it if possible, \
        even on no_std targets. If you still want it off, enable the 'no_safe_jit' feature \
        to get rid of this error."
    );
}

/// Set a `thumb_mode` cfg on arm targets using thumb encoding by default.
///
/// We need this build script to handle Thumb mode for ARM on a stable
/// release channel, where `target_feature = "thumb_mode"` isn't available.
fn set_thumb_mode_cfg() {
    println!("cargo::rustc-check-cfg=cfg(thumb_mode)");

    let arch = var("CARGO_CFG_TARGET_ARCH").unwrap();
    if arch != "arm" {
        return;
    }

    // Detect thumb mode either from target features (if on Nightly)
    // or by parsing the TARGET env var.
    if var("CARGO_CFG_TARGET_FEATURE")
        .map(|f| f.contains("thumb-mode"))
        .unwrap_or_else(|_| {
            let target = var("TARGET").unwrap();
            // In this case, `target-features` can be specified in the custom target spec,
            // which requires nightly to parse
            if target.to_lowercase().ends_with(".json") {
                println!(
                    "cargo::error=Custom ARM target file detected. If using the Thumb ISA by \
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
    if var("CARGO_FEATURE_COVERAGE").is_ok() {
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
    no_safe_jit_warn();
    check_coverage_supported();
    set_thumb_mode_cfg();
}
