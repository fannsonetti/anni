use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/anni.ico");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-env-changed=RC");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let icon = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("assets/anni.ico");
    let source = out.join("anni.rc");
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let major = env::var("CARGO_PKG_VERSION_MAJOR").unwrap();
    let minor = env::var("CARGO_PKG_VERSION_MINOR").unwrap();
    let patch = env::var("CARGO_PKG_VERSION_PATCH").unwrap();
    let icon_path = icon.display().to_string().replace('\\', "/");
    fs::write(
        &source,
        format!(
            r#"1 ICON "{icon_path}"
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "Anni contributors\0"
      VALUE "FileDescription", "Anni school planner\0"
      VALUE "FileVersion", "{version}\0"
      VALUE "InternalName", "anni\0"
      VALUE "OriginalFilename", "anni.exe\0"
      VALUE "ProductName", "Anni\0"
      VALUE "ProductVersion", "{version}\0"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
        ),
    )
    .unwrap();
    let resource = out.join("anni.res");
    let compiler = env::var_os("RC").map(PathBuf::from).unwrap_or_else(|| {
        // Cargo can run outside a Developer Command Prompt. Find the SDK bundled
        // with the same Visual Studio build tools needed for the MSVC linker.
        let root = PathBuf::from(
            env::var_os("ProgramFiles(x86)").unwrap_or_else(|| "C:/Program Files (x86)".into()),
        )
        .join("Windows Kits/10/bin");
        let host = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x64"
        };
        let mut candidates: Vec<_> = fs::read_dir(root)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path().join(host).join("rc.exe"))
            .filter(|path| path.is_file())
            .collect();
        candidates.sort();
        candidates.pop().unwrap_or_else(|| PathBuf::from("rc.exe"))
    });
    let status = Command::new(&compiler)
        .args(["/nologo", "/fo"])
        .arg(&resource)
        .arg(&source)
        .status()
        .expect("Windows SDK resource compiler is required to embed the Anni icon (or set RC)");
    assert!(status.success(), "Could not compile the Anni icon resource");
    println!("cargo:rustc-link-arg-bin=anni={}", resource.display());
}
