//! Build script: embed the Windows executable icon (the one Explorer/taskbar show
//! for the .exe file itself) from `assets/icon.ico`. This is a no-op on every
//! non-Windows target.
//!
//! Note: we intentionally do NOT embed an application manifest requesting
//! Administrator rights — OxideDisk runs unprivileged and self-elevates on demand
//! (see `src/elevation.rs`).

fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/icon.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        if let Err(e) = res.compile() {
            // Don't fail the build on resource-compilation hiccups (e.g. missing
            // rc tooling on an unusual host); the binary just ships without an icon.
            println!("cargo:warning=failed to embed exe icon: {e}");
        }
    }
}
