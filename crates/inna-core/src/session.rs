use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSession {
    pub cookies: Vec<SessionCookie>,
    pub saved_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub http_only: bool,
    pub secure: bool,
}

pub trait SessionStore {
    fn load(&self) -> Result<Option<StoredSession>, SessionError>;
    fn save(&self, session: &StoredSession) -> Result<(), SessionError>;
    fn clear(&self) -> Result<(), SessionError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("session store unavailable")]
    Unavailable,
}

pub struct FileSessionStore {
    path: PathBuf,
}

impl FileSessionStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path() -> PathBuf {
        let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("inna-client").join("session.json")
    }
}

impl SessionStore for FileSessionStore {
    fn load(&self) -> Result<Option<StoredSession>, SessionError> {
        if !self.path.exists() {
            return Ok(None);
        }
        let data = std::fs::read_to_string(&self.path)?;
        let session: StoredSession = serde_json::from_str(&data)?;
        Ok(Some(session))
    }

    fn save(&self, session: &StoredSession) -> Result<(), SessionError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string(session)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&self.path)?;
            std::io::Write::write_all(&mut f, data.as_bytes())?;
            return Ok(());
        }

        #[cfg(not(unix))]
        {
            std::fs::write(&self.path, data)?;
            Ok(())
        }
    }

    fn clear(&self) -> Result<(), SessionError> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }
}
