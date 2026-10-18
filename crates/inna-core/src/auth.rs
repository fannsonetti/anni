use crate::session::StoredSession;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("authentication cancelled by user")]
    Cancelled,
    #[error("authentication helper failed: {0}")]
    HelperFailed(String),
    #[error("could not extract cookies")]
    CookieExtraction,
    #[error("authentication helper not found")]
    HelperNotFound,
    #[error("IPC error: {0}")]
    Ipc(String),
}

/// Result of launching the auth helper process.
pub enum AuthResult {
    Success(StoredSession),
    Cancelled,
    Error(AuthError),
}

/// Launch the separate auth helper process and wait for cookies via stdout IPC.
pub fn launch_auth_helper() -> Result<StoredSession, AuthError> {
    let helper_name = if cfg!(windows) {
        "inna-auth.exe"
    } else {
        "inna-auth"
    };

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    let helper_path = exe_dir
        .as_ref()
        .map(|d| d.join(helper_name))
        .filter(|p| p.exists())
        .unwrap_or_else(|| std::path::PathBuf::from(helper_name));

    if !helper_path.exists() && exe_dir.is_some() {
        // Try PATH
        if which_helper(helper_name).is_none() {
            return Err(AuthError::HelperNotFound);
        }
    }

    tracing::info!("launching auth helper: {:?}", helper_path);

    let output = std::process::Command::new(&helper_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| AuthError::HelperFailed(e.to_string()))?
        .wait_with_output()
        .map_err(|e| AuthError::HelperFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(AuthError::HelperFailed(format!(
            "exit code: {:?}",
            output.status.code()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let session: StoredSession =
        serde_json::from_str(&stdout).map_err(|e| AuthError::Ipc(e.to_string()))?;

    Ok(session)
}

fn which_helper(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join(name))
            .find(|p| p.exists())
    })
}
