use crate::phone_auth::{self, PhoneAuthHandle};
use chrono::Datelike;
use inna_core::client::InnaClient;
use inna_core::models::*;
use inna_core::session::{FileSessionStore, SessionStore, StoredSession};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};

pub enum RuntimeMsg {
    SessionLoaded,
    Connected(User),
    SessionExpired,
    AuthNeeded,
    AuthStarted,
    RestoringSignIn,
    SignInRestoreFailed,
    /// Phone auth server is ready; show this URL in the main UI.
    PhoneAuthReady(String),
    SignInForgotten,
    AuthFailed(String),
    TimetableUpdated(Vec<TimetableEntry>),
    AnnouncementsUpdated(Vec<Announcement>),
    AssignmentsUpdated(Vec<AssignmentModule>),
    HomeworkUpdated(Vec<Homework>),
    TodoUpdated(Vec<TodoItem>),
    SyncError(String, String),
    SyncFinished,
    PingOk,
    Offline,
}

pub struct Runtime {
    tx: mpsc::Sender<RuntimeMsg>,
    // Never dropped: that blocks until spawn_blocking work ends, so closing during the
    // 60 s background sign-in left a windowless Anni holding the single-instance lock.
    rt: std::mem::ManuallyDrop<tokio::runtime::Runtime>,
    automatic_auth_attempted: AtomicBool,
}

impl Runtime {
    pub fn new(tx: mpsc::Sender<RuntimeMsg>) -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("failed to create tokio runtime");
        Self {
            tx,
            rt: std::mem::ManuallyDrop::new(rt),
            automatic_auth_attempted: AtomicBool::new(false),
        }
    }

    pub fn start_session_check(&self, ctx: egui::Context) {
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let store = FileSessionStore::new(FileSessionStore::default_path());

            match store.load() {
                Ok(Some(session)) => {
                    let _ = tx.send(RuntimeMsg::SessionLoaded);

                    let client = match InnaClient::new() {
                        Ok(c) => c,
                        Err(_) => {
                            let _ = tx.send(RuntimeMsg::Offline);
                            ctx.request_repaint();
                            return;
                        }
                    };
                    client.load_session(&session);

                    match client.current_user().await {
                        Ok(user) => {
                            let _ = tx.send(RuntimeMsg::Connected(user));
                        }
                        Err(
                            inna_core::error::InnaError::SessionExpired
                            | inna_core::error::InnaError::NotAuthenticated,
                        ) => {
                            let _ = tx.send(RuntimeMsg::SessionExpired);
                        }
                        Err(_) => {
                            let _ = tx.send(RuntimeMsg::Offline);
                        }
                    }
                }
                _ => {
                    let _ = tx.send(RuntimeMsg::AuthNeeded);
                }
            }
            ctx.request_repaint();
        });
    }

    pub fn try_restore_sign_in(&self, ctx: egui::Context) {
        // A saved session is required: Disconnect must stay disconnected.
        if !inna_core::auth_profile::data_path().is_dir()
            || !matches!(
                FileSessionStore::new(FileSessionStore::default_path()).load(),
                Ok(Some(_))
            )
            || self.automatic_auth_attempted.swap(true, Ordering::SeqCst)
        {
            return;
        }
        let tx = self.tx.clone();
        let _ = tx.send(RuntimeMsg::RestoringSignIn);
        self.rt.spawn(async move {
            let restored = tokio::task::spawn_blocking(|| run_local_helper(true)).await;
            let user = match restored {
                Ok(Ok(session)) => validate_and_save_session(session).await.ok(),
                _ => None,
            };
            let _ = tx.send(match user {
                Some(user) => RuntimeMsg::Connected(user),
                None => RuntimeMsg::SignInRestoreFailed,
            });
            ctx.request_repaint();
        });
    }

    pub fn launch_auth(&self, ctx: egui::Context, phone: bool) {
        let tx = self.tx.clone();
        let _ = tx.send(RuntimeMsg::AuthStarted);

        if phone {
            self.launch_phone_auth(ctx);
        } else {
            self.launch_local_auth(ctx);
        }
    }

    pub fn forget_sign_in(&self, ctx: egui::Context) {
        let tx = self.tx.clone();
        self.rt.spawn_blocking(move || {
            let result = inna_core::auth_profile::forget().and_then(|_| {
                FileSessionStore::new(FileSessionStore::default_path())
                    .clear()
                    .map_err(std::io::Error::other)
            });
            let _ = tx.send(if result.is_ok() {
                RuntimeMsg::SignInForgotten
            } else {
                RuntimeMsg::AuthFailed(
                    "Could not forget your sign-in. Close any Anni sign-in windows and try again."
                        .into(),
                )
            });
            ctx.request_repaint();
        });
    }

    fn launch_phone_auth(&self, ctx: egui::Context) {
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            match phone_auth::start_phone_auth().await {
                Ok(handle) => {
                    let _ = tx.send(RuntimeMsg::PhoneAuthReady(handle.url.clone()));
                    ctx.request_repaint();
                    wait_for_phone_session(handle, tx, ctx).await;
                }
                Err(e) => {
                    let _ = tx.send(RuntimeMsg::AuthFailed(e));
                    ctx.request_repaint();
                }
            }
        });
    }

    fn launch_local_auth(&self, ctx: egui::Context) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            finish_auth_from_helper(run_local_helper(false), tx, ctx);
        });
    }

    pub fn start_sync(
        &self,
        student_id: Option<String>,
        assignments: Vec<AssignmentModule>,
        homework: Vec<Homework>,
        mut timetable: Vec<TimetableEntry>,
        ctx: egui::Context,
    ) {
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let store = FileSessionStore::new(FileSessionStore::default_path());
            let session = match store.load() {
                Ok(Some(s)) => s,
                _ => {
                    let _ = tx.send(RuntimeMsg::SessionExpired);
                    let _ = tx.send(RuntimeMsg::SyncFinished);
                    ctx.request_repaint();
                    return;
                }
            };

            let client = match InnaClient::new() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(RuntimeMsg::SyncError("connection".into(), e.to_string()));
                    let _ = tx.send(RuntimeMsg::SyncFinished);
                    ctx.request_repaint();
                    return;
                }
            };
            client.load_session(&session);

            let now = chrono::Utc::now().date_naive();
            let week_start =
                now - chrono::Duration::days(now.weekday().num_days_from_monday() as i64);
            let week_end = week_start + chrono::Duration::days(13);
            macro_rules! sync_resource {
                ($name:literal, $request:expr, $message:ident) => {
                    match $request.await {
                        Ok(data) => {
                            let _ = tx.send(RuntimeMsg::$message(data));
                        }
                        Err(e) => {
                            let expired = matches!(
                                e,
                                inna_core::error::InnaError::SessionExpired
                                    | inna_core::error::InnaError::NotAuthenticated
                            );
                            let offline = matches!(e, inna_core::error::InnaError::Network);
                            let _ = tx.send(RuntimeMsg::SyncError($name.into(), e.to_string()));
                            if offline {
                                let _ = tx.send(RuntimeMsg::Offline);
                            }
                            if expired {
                                let _ = tx.send(RuntimeMsg::SessionExpired);
                            }
                            if expired || offline {
                                let _ = tx.send(RuntimeMsg::SyncFinished);
                                ctx.request_repaint();
                                return;
                            }
                        }
                    }
                    ctx.request_repaint();
                };
            }
            if let Some(sid) = student_id.as_deref().filter(|s| !s.is_empty()) {
                sync_resource!(
                    "timetable",
                    async {
                        let rows = client.timetable(sid, week_start, week_end).await?;
                        timetable = rows.clone();
                        Ok::<_, inna_core::error::InnaError>(rows)
                    },
                    TimetableUpdated
                );
            } else {
                let _ = tx.send(RuntimeMsg::SyncError(
                    "timetable".into(),
                    "INNA did not return a student ID".into(),
                ));
            }
            match client
                .schoolwork(assignments, homework, &timetable, now)
                .await
            {
                Ok(work) => {
                    let _ = tx.send(RuntimeMsg::AssignmentsUpdated(work.assignments));
                    let _ = tx.send(RuntimeMsg::HomeworkUpdated(work.homework));
                    for (resource, warning) in work.warnings {
                        let _ = tx.send(RuntimeMsg::SyncError(resource, warning));
                    }
                }
                Err(error) => {
                    let expired = matches!(
                        error,
                        inna_core::error::InnaError::SessionExpired
                            | inna_core::error::InnaError::NotAuthenticated
                    );
                    let _ = tx.send(RuntimeMsg::SyncError(
                        "assignments".into(),
                        error.to_string(),
                    ));
                    let _ = tx.send(RuntimeMsg::SyncError("homework".into(), error.to_string()));
                    let _ = tx.send(if expired {
                        RuntimeMsg::SessionExpired
                    } else {
                        RuntimeMsg::Offline
                    });
                    let _ = tx.send(RuntimeMsg::SyncFinished);
                    ctx.request_repaint();
                    return;
                }
            }
            sync_resource!("todo", client.todo(false), TodoUpdated);
            sync_resource!(
                "announcements",
                client.announcements(),
                AnnouncementsUpdated
            );
            let _ = tx.send(RuntimeMsg::SyncFinished);
            ctx.request_repaint();
        });
    }

    pub fn start_ping(&self, ctx: egui::Context) {
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let store = FileSessionStore::new(FileSessionStore::default_path());
            let session = match store.load() {
                Ok(Some(s)) => s,
                _ => {
                    let _ = tx.send(RuntimeMsg::SessionExpired);
                    ctx.request_repaint();
                    return;
                }
            };

            let client = match InnaClient::new() {
                Ok(c) => c,
                Err(_) => return,
            };
            client.load_session(&session);

            match client.ping().await {
                Ok(_) => {
                    let _ = tx.send(RuntimeMsg::PingOk);
                }
                Err(inna_core::error::InnaError::SessionExpired) => {
                    let _ = tx.send(RuntimeMsg::SessionExpired);
                }
                Err(_) => {
                    let _ = tx.send(RuntimeMsg::Offline);
                }
            }
            ctx.request_repaint();
        });
    }
}

async fn wait_for_phone_session(
    handle: PhoneAuthHandle,
    tx: mpsc::Sender<RuntimeMsg>,
    ctx: egui::Context,
) {
    loop {
        if handle.is_done() {
            if let Some(session) = handle.take_session() {
                apply_session(session, &tx).await;
            } else {
                let _ = tx.send(RuntimeMsg::AuthFailed(
                    "phone auth finished without a session".into(),
                ));
            }
            ctx.request_repaint();
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        ctx.request_repaint();
    }
}

async fn apply_session(session: StoredSession, tx: &mpsc::Sender<RuntimeMsg>) {
    let _ = tx.send(match validate_and_save_session(session).await {
        Ok(user) => RuntimeMsg::Connected(user),
        Err(error) => RuntimeMsg::AuthFailed(error),
    });
}

async fn validate_and_save_session(session: StoredSession) -> Result<User, String> {
    let client = InnaClient::new().map_err(|error| error.to_string())?;
    client.load_session(&session);

    match client.current_user().await {
        Ok(user) => {
            let store = FileSessionStore::new(FileSessionStore::default_path());
            if store.save(&session).is_err() {
                return Err("Could not save your INNA sign-in on this computer.".into());
            }
            Ok(user)
        }
        Err(e) => Err(format!("session validation failed: {e}")),
    }
}

fn finish_auth_from_helper(
    result: Result<StoredSession, String>,
    tx: mpsc::Sender<RuntimeMsg>,
    ctx: egui::Context,
) {
    match result {
        Ok(session) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(apply_session(session, &tx));
        }
        Err(e) => {
            let _ = tx.send(RuntimeMsg::AuthFailed(e));
        }
    }
    ctx.request_repaint();
}

fn run_local_helper(silent: bool) -> Result<StoredSession, String> {
    let mut command = Command::new(auth_helper_path());
    if silent {
        command.arg("--silent");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW; native sign-in UI still works.
    }
    let child = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|error| format!("failed to launch auth helper: {error}"))?;
    wait_for_helper(child, silent.then_some(std::time::Duration::from_secs(60)))
}

fn wait_for_helper(
    mut child: std::process::Child,
    timeout: Option<std::time::Duration>,
) -> Result<StoredSession, String> {
    // Drain stdout while polling: a session can exceed the Windows pipe buffer.
    let mut stdout = child.stdout.take().ok_or("Missing sign-in output")?;
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    if let Some(timeout) = timeout {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(
                        "Remembered sign-in needs attention. Reconnect in Anni to continue.".into(),
                    );
                }
            }
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err("Sign-in did not complete. Reconnect in Anni to try again.".into());
    }
    let bytes = reader
        .join()
        .map_err(|_| "Could not read the sign-in result")?
        .map_err(|_| "Could not read the sign-in result")?;
    serde_json::from_slice(&bytes)
        .map_err(|_| "Could not read the sign-in result. Please try again.".into())
}

fn auth_helper_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap_or_default();
    path.pop();

    #[cfg(windows)]
    {
        path.push("inna-auth.exe");
    }
    #[cfg(not(windows))]
    {
        path.push("inna-auth");
    }

    path
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::time::{Duration, Instant};

    fn child(script: &str) -> std::process::Child {
        Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .creation_flags(0x0800_0000)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap()
    }

    #[test]
    fn dropping_the_runtime_does_not_wait_for_a_background_sign_in() {
        let runtime = Runtime::new(mpsc::channel().0);
        runtime
            .rt
            .spawn_blocking(|| std::thread::sleep(Duration::from_secs(30)));
        let started = Instant::now();
        drop(runtime);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn background_helper_timeout_stops_a_stuck_process() {
        let started = Instant::now();
        let result = wait_for_helper(
            child("Start-Sleep -Seconds 30"),
            Some(Duration::from_millis(150)),
        );
        assert!(result.unwrap_err().contains("Reconnect in Anni"));
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn background_helper_drains_sessions_larger_than_the_pipe_buffer() {
        let session = wait_for_helper(
            child(r#"[Console]::Out.Write('{"cookies":[],"saved_at":"' + ('x' * 65536) + '"}')"#),
            Some(Duration::from_secs(15)),
        )
        .unwrap();
        assert_eq!(session.saved_at.len(), 65536);
        assert!(wait_for_helper(
            child("[Console]::Out.Write('invalid')"),
            Some(Duration::from_secs(15))
        )
        .is_err());
        assert!(wait_for_helper(child("exit 1"), Some(Duration::from_secs(15))).is_err());
    }
}
