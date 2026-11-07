fn main() {
    // libgit2-sys links Security/CoreFoundation unconditionally; loading unused frameworks costs ~2ms per run.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg-bins=-Wl,-dead_strip_dylibs");
    }
}
