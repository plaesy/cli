use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=CC");
    if std::env::var("CARGO_CFG_TARGET_OS").ok().as_deref() != Some("macos") {
        return;
    }

    let runtime_dir = clang_output("-print-runtime-dir")
        .filter(|dir| dir.join("libclang_rt.osx.a").is_file())
        .or_else(|| {
            clang_output("-print-resource-dir")
                .map(|resource| resource.join("lib").join("darwin"))
                .filter(|dir| dir.join("libclang_rt.osx.a").is_file())
        });
    let Some(runtime_dir) = runtime_dir else {
        println!("cargo:warning=Apple Clang compiler-rt library libclang_rt.osx.a was not found");
        return;
    };

    // ONNX Runtime's macOS archive uses Clang's availability-check runtime. Rust's release/LTO
    // link omits it by default, leaving ___isPlatformVersionAtLeast unresolved on current Xcode.
    println!("cargo:rustc-link-search=native={}", runtime_dir.display());
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
}

fn clang_output(option: &str) -> Option<PathBuf> {
    let output = Command::new("clang").arg(option).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?;
    let path = path.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}
