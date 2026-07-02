use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=assets/icons/app.ico");

    if env::var("CARGO_CFG_WINDOWS").is_err() {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let resource = out_dir.join("dodbogi.res");
    let rc = find_resource_compiler().unwrap_or_else(|| PathBuf::from("rc.exe"));
    let status = Command::new(&rc)
        .current_dir(&manifest_dir)
        .arg("/nologo")
        .arg(format!("/fo{}", resource.display()))
        .arg("app.rc")
        .status()
        .unwrap_or_else(|error| {
            panic!(
                "failed to run Windows resource compiler {}: {error}",
                rc.display()
            )
        });
    if !status.success() {
        panic!("Windows resource compiler failed with status {status}");
    }

    println!("cargo:rustc-link-arg-bin=dodbogi={}", resource.display());
}

fn find_resource_compiler() -> Option<PathBuf> {
    if let Ok(path) = env::var("RC") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    let arch = match env::var("TARGET").unwrap_or_default().as_str() {
        target if target.contains("i686") => "x86",
        target if target.contains("aarch64") => "arm64",
        _ => "x64",
    };
    let kits_root = env::var("ProgramFiles(x86)")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(r"C:\Program Files (x86)"))
        .join("Windows Kits")
        .join("10")
        .join("bin");
    let mut candidates = Vec::new();
    if let Ok(entries) = fs::read_dir(&kits_root) {
        for entry in entries.flatten() {
            let path = entry.path().join(arch).join("rc.exe");
            if path.is_file() {
                candidates.push(path);
            }
        }
    }
    candidates.sort();
    candidates.pop()
}
