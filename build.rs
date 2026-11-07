fn main() {
    // libgit2-sys links Security/CoreFoundation unconditionally; loading unused frameworks costs ~2ms per run.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg-bins=-Wl,-dead_strip_dylibs");
        // Rust targets macOS 11, so ld64 defaults to classic rebase opcodes for the ~357KB of static
        // pointer tables in __DATA_CONST; chained fixups cut ~2M instructions from every launch.
        println!("cargo:rustc-link-arg-bins=-Wl,-fixup_chains");
    }
}
