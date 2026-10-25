use crate::phone_auth::{self, PhoneAuthHandle};
use chrono::Datelike;
use inna_core::client::InnaClient;
use inna_core::models::*;
use inna_core::session::{FileSessionStore, SessionStore, StoredSession};
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;

pub enum RuntimeMsg {
    SessionLoaded,
    Connected(User),
    SessionExpired,
    AuthNeeded,
    AuthStarted,
    /// Phone auth server is ready; show this URL in the main UI.
    PhoneAuthReady(String),
    BrowserAuthReady(String),
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
    rt: tokio::runtime::Runtime,
    browser_auth: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
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
            rt,
            browser_auth: std::sync::Mutex::new(None),
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

    pub fn launch_auth(&self, ctx: egui::Context, phone: bool) {
        let tx = self.tx.clone();
        let _ = tx.send(RuntimeMsg::AuthStarted);

        if phone {
            self.launch_phone_auth(ctx);
        } else {
            self.launch_local_auth(ctx);
        }
    }

    pub fn launch_browser_auth(&self, ctx: egui::Context) {
        self.cancel_browser_auth();
        let tx = self.tx.clone();
        let _ = tx.send(RuntimeMsg::AuthStarted);
        let task = self.rt.spawn(async move {
            let result = match crate::browser_auth::start().await {
                Ok(handle) => {
                    let _ = tx.send(RuntimeMsg::BrowserAuthReady(handle.url.clone()));
                    ctx.open_url(egui::OpenUrl::new_tab(&handle.url));
                    ctx.request_repaint();
                    handle.wait().await
                }
                Err(error) => Err(error),
            };
            match result {
                Ok(session) => apply_session(session, &tx).await,
                Err(error) => {
                    let _ = tx.send(RuntimeMsg::AuthFailed(error));
                }
            }
            ctx.request_repaint();
        });
        *self.browser_auth.lock().unwrap() = Some(task);
    }

    pub fn cancel_browser_auth(&self) {
        if let Some(task) = self.browser_auth.lock().unwrap().take() {
            task.abort();
        }
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
            let auth_exe = auth_helper_path();
            tracing::info!("launching auth helper: {}", auth_exe.display());

            let output = Command::new(&auth_exe).output();
            finish_auth_from_helper(output, tx, ctx);
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
    let client = match InnaClient::new() {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(RuntimeMsg::AuthFailed(e.to_string()));
            return;
        }
    };
    client.load_session(&session);

    match client.current_user().await {
        Ok(user) => {
            let store = FileSessionStore::new(FileSessionStore::default_path());
            if store.save(&session).is_err() {
                let _ = tx.send(RuntimeMsg::AuthFailed(
                    "Could not save your INNA sign-in on this computer.".into(),
                ));
                return;
            }
            let _ = tx.send(RuntimeMsg::Connected(user));
        }
        Err(e) => {
            let _ = tx.send(RuntimeMsg::AuthFailed(format!(
                "session validation failed: {e}"
            )));
        }
    }
}

fn finish_auth_from_helper(
    output: std::io::Result<std::process::Output>,
    tx: mpsc::Sender<RuntimeMsg>,
    ctx: egui::Context,
) {
    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            match serde_json::from_str::<StoredSession>(stdout.trim()) {
                Ok(session) => {
                    let store = FileSessionStore::new(FileSessionStore::default_path());
                    if let Err(e) = store.save(&session) {
                        tracing::error!("failed to save session: {e}");
                    }

                    let client = match InnaClient::new() {
                        Ok(c) => c,
                        Err(e) => {
                            let _ = tx.send(RuntimeMsg::AuthFailed(e.to_string()));
                            ctx.request_repaint();
                            return;
                        }
                    };
                    client.load_session(&session);

                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();
                    match rt.block_on(client.current_user()) {
                        Ok(user) => {
                            let _ = tx.send(RuntimeMsg::Connected(user));
                        }
                        Err(e) => {
                            let _ = tx.send(RuntimeMsg::AuthFailed(format!(
                                "session validation failed: {e}"
                            )));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(RuntimeMsg::AuthFailed(format!(
                        "failed to parse auth output: {e}"
                    )));
                }
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let _ = tx.send(RuntimeMsg::AuthFailed(format!(
                "auth helper exited with {}: {}",
                out.status,
                stderr.trim()
            )));
        }
        Err(e) => {
            let _ = tx.send(RuntimeMsg::AuthFailed(format!(
                "failed to launch auth helper: {e}"
            )));
        }
    }
    ctx.request_repaint();
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
