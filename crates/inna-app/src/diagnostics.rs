//! Anonymous bug reports queued locally and sent hourly to the owner's storage PC.
//! Only typed events are accepted; raw errors, school records and credentials are excluded.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

const DESTINATION: &str = "http://85.220.66.209:3000/anni/diagnostics";
const LIMIT: usize = 500;
static REPORTER: OnceLock<Arc<Reporter>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    Connection,
    Timetable,
    Assignments,
    Homework,
    Todo,
    Announcements,
    Storage,
    Desktop,
    Update,
}

impl Resource {
    pub fn from_name(name: &str) -> Self {
        match name {
            "timetable" => Self::Timetable,
            "assignments" => Self::Assignments,
            "homework" => Self::Homework,
            "todo" => Self::Todo,
            "announcements" => Self::Announcements,
            _ => Self::Connection,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    Network,
    SessionExpired,
    Parse,
    Http,
    Storage,
    Other,
}

impl Failure {
    pub fn classify(error: &str) -> Self {
        let error = error.to_ascii_lowercase();
        if error.contains("network") || error.contains("timed out") {
            Self::Network
        } else if error.contains("session expired") || error.contains("not authenticated") {
            Self::SessionExpired
        } else if error.contains("parse") || error.contains("unexpected inna response") {
            Self::Parse
        } else if error.contains("http status") {
            Self::Http
        } else {
            Self::Other
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Started,
    Connected,
    PlannerHidden,
    Quit,
    Failure {
        resource: Resource,
        category: Failure,
    },
    SyncSummary {
        lessons: usize,
        courses: usize,
        homework: usize,
        warnings: usize,
        generic_course_labels: usize,
        non_lesson_entries: usize,
        invalid_lesson_times: usize,
    },
    Panic {
        line: u32,
        column: u32,
    },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: u64,
    time: String,
    version: String,
    os: String,
    arch: String,
    event: Event,
}

#[derive(Serialize, Deserialize)]
struct Journal {
    device_id: String,
    share: bool,
    storage_computer: bool,
    next_id: u64,
    sent_through: u64,
    entries: VecDeque<Entry>,
}

impl Journal {
    fn new() -> Self {
        // No hardware fingerprint, computer name or INNA identifier.
        let seed = format!("{:?}-{}", std::time::SystemTime::now(), std::process::id());
        Self {
            device_id: format!("{:x}", Sha256::digest(seed.as_bytes()))[..32].into(),
            share: true,
            storage_computer: directly_owns_public_ip(),
            next_id: 1,
            sent_through: 0,
            entries: VecDeque::new(),
        }
    }

    fn push(&mut self, event: Event) {
        if self.entries.back().is_some_and(|last| {
            last.event == event
                && chrono::DateTime::parse_from_rfc3339(&last.time).is_ok_and(|time| {
                    chrono::Utc::now().signed_duration_since(time).num_seconds() < 300
                })
        }) {
            return;
        }
        self.entries.push_back(Entry {
            id: self.next_id,
            time: chrono::Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").into(),
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            event,
        });
        self.next_id += 1;
        while self.entries.len() > LIMIT {
            self.entries.pop_front();
        }
    }

    fn batch(&self) -> Vec<Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.id > self.sent_through)
            .take(40)
            .cloned()
            .collect()
    }

    fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(temp, path)
    }
}

struct Reporter {
    journal: Mutex<Journal>,
    path: PathBuf,
    status: Mutex<String>,
    server_status: Mutex<String>,
}

pub fn path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("inna-client/diagnostics.json")
}

fn directly_owns_public_ip() -> bool {
    // This does not contact the Internet. Behind a router, select the storage PC in Settings.
    std::net::UdpSocket::bind(("85.220.66.209", 0)).is_ok()
}

pub fn init(mock: bool, storage_computer: bool) {
    if mock {
        return;
    }
    let path = path();
    let mut journal: Journal = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(Journal::new);
    if storage_computer {
        journal.storage_computer = true;
    }
    let reporter = Arc::new(Reporter {
        journal: Mutex::new(journal),
        path,
        status: Mutex::new("Waiting for the hourly upload".into()),
        server_status: Mutex::new("Storage is off".into()),
    });
    if REPORTER.set(reporter.clone()).is_err() {
        return;
    }
    record(Event::Started);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let (line, column) = info
            .location()
            .map(|location| (location.line(), location.column()))
            .unwrap_or_default();
        // Panic messages may contain data or credentials. Record the code location only.
        record(Event::Panic { line, column });
        previous(info);
    }));
    let _ = std::thread::Builder::new()
        .name("anni-diagnostics".into())
        .spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            runtime.block_on(async {
                let collector = reporter.clone();
                tokio::spawn(async move {
                    collector.serve().await;
                });
                let Ok(client) = reqwest::Client::builder()
                    .timeout(Duration::from_secs(10))
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                else {
                    return;
                };
                // Allow login/restore diagnostics to join the first batch.
                tokio::time::sleep(Duration::from_secs(15)).await;
                loop {
                    reporter.upload(&client, DESTINATION).await;
                    tokio::time::sleep(Duration::from_secs(3600)).await;
                }
            });
        });
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Batch {
    schema: u8,
    device_id: String,
    events: Vec<Entry>,
}

impl Reporter {
    async fn upload(&self, client: &reqwest::Client, destination: &str) {
        // Drain the bounded queue once per hour, stopping immediately if storage is offline.
        for _ in 0..LIMIT.div_ceil(40) {
            let batch = {
                let Ok(journal) = self.journal.lock() else {
                    return;
                };
                if !journal.share {
                    return;
                }
                Batch {
                    schema: 1,
                    device_id: journal.device_id.clone(),
                    events: journal.batch(),
                }
            };
            let Some(last) = batch.events.last().map(|entry| entry.id) else {
                return;
            };
            // The storage PC writes its own reports directly, even without router hairpin NAT.
            if self.hosting() {
                let folder = self.path.parent().unwrap().join("received-reports");
                let own_batch = Batch {
                    schema: batch.schema,
                    device_id: batch.device_id.clone(),
                    events: batch.events.clone(),
                };
                let result =
                    tokio::task::spawn_blocking(move || store_batch(&folder, &own_batch)).await;
                if !matches!(result, Ok(Ok(()))) {
                    *self.status.lock().unwrap() = "Could not store this computer's reports".into();
                    return;
                }
            } else {
                let result = client.post(destination).json(&batch).send().await;
                if !result
                    .is_ok_and(|response| response.status() == reqwest::StatusCode::NO_CONTENT)
                {
                    *self.status.lock().unwrap() =
                        "Storage is offline or unavailable; retrying in one hour".into();
                    return;
                }
            }
            if let Ok(mut journal) = self.journal.lock() {
                journal.sent_through = journal.sent_through.max(last);
                if journal.save(&self.path).is_err() {
                    *self.status.lock().unwrap() = "Could not save delivery status".into();
                    return;
                }
            }
            *self.status.lock().unwrap() = "Reports delivered".into();
        }
    }

    fn hosting(&self) -> bool {
        self.journal
            .lock()
            .map(|journal| journal.storage_computer)
            .unwrap_or(false)
    }

    async fn serve(self: Arc<Self>) {
        use axum::{
            routing::{get, post},
            Router,
        };
        loop {
            if self.hosting() {
                match tokio::net::TcpListener::bind("0.0.0.0:3000").await {
                    Ok(listener) => {
                        *self.server_status.lock().unwrap() =
                            "Receiving bug reports on port 3000".into();
                        let folder = self.path.parent().unwrap().join("received-reports");
                        let router = Router::new()
                            .route("/anni/health", get(|| async { "anni-diagnostics-v1" }))
                            .route("/anni/diagnostics", post(receive))
                            .layer(axum::extract::DefaultBodyLimit::max(64 * 1024))
                            .with_state(folder);
                        let control = self.clone();
                        let _ = axum::serve(listener, router)
                            .with_graceful_shutdown(async move {
                                while control.hosting() {
                                    tokio::time::sleep(Duration::from_secs(2)).await;
                                }
                            })
                            .await;
                    }
                    Err(_) => {
                        *self.server_status.lock().unwrap() =
                            "Could not listen on port 3000; another program may be using it".into();
                    }
                }
            } else {
                *self.server_status.lock().unwrap() = "Storage is off".into();
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
}

fn valid_batch(batch: &Batch) -> bool {
    batch.schema == 1
        && batch.device_id.len() == 32
        && batch.device_id.bytes().all(|byte| byte.is_ascii_hexdigit())
        && !batch.events.is_empty()
        && batch.events.len() <= 40
        && batch.events.windows(2).all(|pair| pair[0].id < pair[1].id)
        && batch.events.iter().all(|entry| {
            entry.id > 0
                && entry.version.len() <= 24
                && semver::Version::parse(&entry.version).is_ok()
                && ["windows", "linux", "macos"].contains(&entry.os.as_str())
                && ["x86_64", "x86", "aarch64", "arm"].contains(&entry.arch.as_str())
                && entry.time.len() <= 40
                && chrono::DateTime::parse_from_rfc3339(&entry.time).is_ok()
        })
}

async fn receive(
    axum::extract::State(folder): axum::extract::State<PathBuf>,
    axum::Json(batch): axum::Json<Batch>,
) -> axum::http::StatusCode {
    if !valid_batch(&batch) {
        return axum::http::StatusCode::BAD_REQUEST;
    }
    // Serialize disk writes and retention checks; acknowledge only after a durable write.
    static WRITER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = WRITER.lock().await;
    let result = tokio::task::spawn_blocking(move || store_batch(&folder, &batch)).await;
    if matches!(result, Ok(Ok(()))) {
        axum::http::StatusCode::NO_CONTENT
    } else {
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    }
}

fn store_batch(folder: &Path, batch: &Batch) -> std::io::Result<()> {
    std::fs::create_dir_all(folder)?;
    // One file per batch; retries overwrite the same file, avoiding duplicate reports.
    let file = folder.join(format!(
        "{}-{}-{}.json",
        batch.device_id,
        batch.events[0].id,
        batch.events.last().unwrap().id
    ));
    let temp = file.with_extension("tmp");
    let mut output = std::fs::File::create(&temp)?;
    use std::io::Write;
    output.write_all(&serde_json::to_vec_pretty(batch)?)?;
    output.sync_all()?;
    std::fs::rename(temp, file)?;
    // Keep at most 2,000 reports, pruning reports older than 30 days as well.
    let mut files: Vec<_> = std::fs::read_dir(folder)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    files.sort_by_key(|(modified, _)| *modified);
    let excess = files.len().saturating_sub(2000);
    for (i, (modified, file)) in files.into_iter().enumerate() {
        if i < excess
            || modified
                .elapsed()
                .is_ok_and(|age| age > Duration::from_secs(30 * 86400))
        {
            let _ = std::fs::remove_file(file);
        }
    }
    Ok(())
}

pub fn record(event: Event) {
    if let Some(reporter) = REPORTER.get() {
        // The panic hook must not deadlock if a writer panics while holding the journal.
        if let Ok(mut journal) = reporter.journal.try_lock() {
            journal.push(event);
            if journal.save(&reporter.path).is_err() {
                *reporter.status.lock().unwrap() = "Could not save diagnostic logs".into();
            }
        }
    }
}

pub fn failure(resource: Resource, category: Failure) {
    record(Event::Failure { resource, category });
}

pub fn settings_ui(ui: &mut egui::Ui, mock: bool) {
    crate::ui::theme::card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(egui::RichText::new("Bug reports").size(20.0).strong());
        ui.label("Logs help find refresh failures, missing course labels, and calendar entries mistaken for classes. They include an anonymous device ID, app version, counts, and error categories. Names, grades, full schedules, passwords, and cookies are excluded.");
        if mock { ui.label("Demo mode: bug logging is disabled."); return; }
        if let Some(reporter) = REPORTER.get() {
            let mut journal = reporter.journal.lock().unwrap();
            let changed = ui.checkbox(&mut journal.share, "Send bug reports when the storage computer is online").changed();
            let host_changed = ui.checkbox(&mut journal.storage_computer, "Use this computer to store everyone's bug reports").changed();
            if (changed || host_changed) && journal.save(&reporter.path).is_err() {
                *reporter.status.lock().unwrap() = "Could not save bug report preferences".into();
            }
            ui.label("Reports wait here and retry every hour at 85.220.66.209:3000. The latest 500 events are kept on each computer.");
            ui.label(if journal.share { reporter.status.lock().unwrap().clone() } else { "Sharing is off. Logs stay here.".into() });
            if journal.storage_computer {
                ui.label(reporter.server_status.lock().unwrap().clone());
                ui.label("Forward your router's TCP port 3000 to this PC and allow Anni through Windows Firewall. Keep Anni or its desktop widgets running to receive reports.");
                ui.label(format!("Reports folder: {}", reporter.path.parent().unwrap().join("received-reports").display()));
            }
            if ui.button("Copy diagnostic log").clicked() {
                if let Ok(log) = serde_json::to_string_pretty(&*journal) { ui.output_mut(|output| output.copied_text = log); }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn summary(lessons: usize) -> Event {
        Event::SyncSummary {
            lessons,
            courses: 2,
            homework: 1,
            warnings: 0,
            generic_course_labels: 0,
            non_lesson_entries: 0,
            invalid_lesson_times: 0,
        }
    }

    #[test]
    fn bounded_queue_survives_restart_and_preserves_unacknowledged_events() {
        let path =
            std::env::temp_dir().join(format!("anni-diagnostics-{}.json", std::process::id()));
        let mut journal = Journal::new();
        for i in 0..550 {
            journal.push(summary(i));
        }
        journal.save(&path).unwrap();
        let mut restored: Journal = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(restored.device_id, journal.device_id);
        assert_eq!(restored.entries.len(), LIMIT);
        let batch = restored.batch();
        assert_eq!(batch.len(), 40);
        restored.push(Event::Quit);
        restored.sent_through = batch.last().unwrap().id;
        assert_eq!(restored.batch()[0].id, restored.sent_through + 1);
        assert_eq!(restored.entries.back().unwrap().event, Event::Quit);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn raw_errors_and_school_identifiers_cannot_enter_reports() {
        let private =
            "Course SECRET123: failed to parse INNA response; cookie=SECRET; student=12345";
        let mut journal = Journal::new();
        journal.push(Event::Failure {
            resource: Resource::from_name(private),
            category: Failure::classify(private),
        });
        let json = serde_json::to_string(&journal).unwrap();
        assert!(!json.contains("SECRET"));
        assert!(!json.contains("12345"));
        assert!(json.contains("parse"));
        journal.push(Event::Failure {
            resource: Resource::Connection,
            category: Failure::Parse,
        });
        assert_eq!(journal.entries.len(), 1);
    }

    #[tokio::test]
    async fn upload_acknowledges_only_stored_batches_and_retries_offline_queue() {
        let root =
            std::env::temp_dir().join(format!("anni-diagnostics-transport-{}", std::process::id()));
        let mut journal = Journal::new();
        journal.storage_computer = false;
        journal.push(summary(10));
        journal.push(Event::Quit);
        let reporter = Reporter {
            journal: Mutex::new(journal),
            path: root.join("queue.json"),
            status: Mutex::new(String::new()),
            server_status: Mutex::new(String::new()),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let router = axum::Router::new()
            .route("/anni/diagnostics", axum::routing::post(receive))
            .with_state(root.join("received"));
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(1))
            .build()
            .unwrap();
        reporter
            .upload(&client, &format!("http://{addr}/missing"))
            .await;
        assert_eq!(reporter.journal.lock().unwrap().sent_through, 0);
        let endpoint = format!("http://{addr}/anni/diagnostics");
        reporter.upload(&client, &endpoint).await;
        assert_eq!(reporter.journal.lock().unwrap().sent_through, 2);
        assert_eq!(std::fs::read_dir(root.join("received")).unwrap().count(), 1);
        reporter.journal.lock().unwrap().sent_through = 0;
        reporter.upload(&client, &endpoint).await;
        assert_eq!(std::fs::read_dir(root.join("received")).unwrap().count(), 1);
        let mut invalid = Batch {
            schema: 1,
            device_id: "../escape".into(),
            events: reporter.journal.lock().unwrap().batch(),
        };
        assert!(!valid_batch(&invalid));
        invalid.device_id = "a".repeat(32);
        assert!(!valid_batch(&invalid)); // empty batch
        server.abort();
        std::fs::remove_dir_all(root).unwrap();
    }
}
