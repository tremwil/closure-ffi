// We need this build script to handle Thumb mode for ARM on a stable
// release channel, where `target_feature = "thumb_mode"` isn't available.
fn main() {
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
                    r"cargo::warning=Custom ARM target file detected. If using the Thumb ISA by 
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
