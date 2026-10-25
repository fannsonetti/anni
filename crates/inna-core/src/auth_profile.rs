//! Anni's own persistent sign-in profile; never uses a Chrome or Edge user profile.
use std::{
    io,
    path::{Path, PathBuf},
};

fn stable_path() -> PathBuf {
    crate::session::FileSessionStore::default_path()
        .parent()
        .unwrap()
        .join("sign-in")
}

fn legacy_path() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::current_exe()
            .ok()?
            .parent()
            .map(|parent| parent.join("inna-auth.exe.WebView2"))
    } else {
        None
    }
}

fn select(stable: PathBuf, legacy: Option<PathBuf>) -> PathBuf {
    // Keep an already remembered account from older Anni helpers, without copying cookies.
    if !stable.exists() {
        if let Some(legacy) = legacy.filter(|path| path.is_dir()) {
            return legacy;
        }
    }
    stable
}

pub fn data_path() -> PathBuf {
    select(stable_path(), legacy_path())
}

pub fn prepare() -> io::Result<PathBuf> {
    let path = data_path();
    std::fs::create_dir_all(&path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(path)
}

fn remove_owned_profile(path: &Path) -> io::Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    // Only remove the exact direct-child directory chosen above, never a junction or symlink target.
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing profile parent"))?
        .canonicalize()?;
    let resolved = path.canonicalize()?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || resolved.parent() != Some(parent.as_path())
    {
        return Err(io::Error::other("Unexpected sign-in profile location"));
    }
    std::fs::remove_dir_all(resolved)
}

pub fn forget() -> io::Result<()> {
    remove_owned_profile(&stable_path())?;
    if let Some(legacy) = legacy_path() {
        remove_owned_profile(&legacy)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_anni_profile_is_reused_and_forgetting_only_removes_that_directory() {
        let root = std::env::temp_dir().join(format!("anni-profile-test-{}", std::process::id()));
        let stable = root.join("sign-in");
        let legacy = root.join("inna-auth.exe.WebView2");
        let plan = root.join("planner.json");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(&plan, "saved plan").unwrap();
        assert_eq!(select(stable.clone(), Some(legacy.clone())), legacy);
        std::fs::create_dir_all(&stable).unwrap();
        std::fs::write(stable.join("cookie-test"), "synthetic").unwrap();
        assert_eq!(select(stable.clone(), Some(legacy.clone())), stable);
        remove_owned_profile(&stable).unwrap();
        assert!(legacy.is_dir());
        assert_eq!(std::fs::read_to_string(&plan).unwrap(), "saved plan");
        remove_owned_profile(&legacy).unwrap();
        assert_eq!(select(stable.clone(), Some(legacy)), stable);
        assert!(root.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(root).unwrap();
    }
}
