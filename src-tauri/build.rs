fn main() {
    #[cfg(target_os = "macos")]
    {
        use std::{env, path::PathBuf, process::Command};
        let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("StarPersonaDock");
        let arch = if env::var("CARGO_CFG_TARGET_ARCH").unwrap() == "aarch64" {
            "arm64"
        } else {
            "x86_64"
        };
        println!("cargo:rerun-if-changed=src/dock.m");
        assert!(Command::new("/usr/bin/clang")
            .args([
                "-arch",
                arch,
                "-mmacosx-version-min=11.0",
                "-O2",
                "-fobjc-arc",
                "-framework",
                "AppKit",
                "src/dock.m",
                "-o"
            ])
            .arg(out)
            .status()
            .unwrap()
            .success());
    }
    tauri_build::build()
}
