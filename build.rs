fn main() {
    println!("cargo::rustc-check-cfg=cfg(thumb_mode)");

    if std::env::var("CARGO_CFG_TARGET_ARCH").unwrap() != "arm" {
        return;
    }

    let target = std::env::var("TARGET").unwrap();
    if target.starts_with("thumb") {
        println!("cargo:rustc-cfg=thumb_mode");
    }
}