//! Startup updates use the published Windows installer, never replace a running EXE.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

pub const RELEASES_URL: &str = "https://api.github.com/repos/fannsonetti/anni/releases/latest";
const DOWNLOAD_ROOT: &str = "https://github.com/fannsonetti/anni/releases/download/";
const MAX_INSTALLER: u64 = 150 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Debug, Clone, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}
#[derive(Debug)]
struct Available {
    version: String,
    installer: Asset,
    checksum: Asset,
}
#[derive(Debug, Clone)]
pub struct Staged {
    pub version: String,
    installer: PathBuf,
    checksum: String,
}
#[derive(Debug, Clone)]
pub enum Progress {
    Checking,
    Downloading {
        version: String,
        received: u64,
        total: u64,
    },
    Verifying {
        version: String,
    },
    Current,
    Staged(Staged),
    Installing {
        version: String,
    },
    Complete,
    Failed(String),
}

pub fn should_check(mock: bool, skip: bool) -> bool {
    cfg!(windows)
        && !cfg!(debug_assertions)
        && !mock
        && !skip
        && std::env::current_exe()
            .ok()
            .is_some_and(|exe| installed(&exe))
}
fn installed(exe: &Path) -> bool {
    exe.file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("anni.exe"))
        && exe
            .parent()
            .is_some_and(|dir| dir.join("unins000.exe").is_file())
}
fn cache_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("inna-client")
        .join("updates")
}
fn available(release: Release, current: &str) -> Result<Option<Available>, String> {
    let version = semver::Version::parse(
        release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name),
    )
    .map_err(|_| "The release has an invalid version.")?;
    let current =
        semver::Version::parse(current).map_err(|_| "The installed version is invalid.")?;
    if release.draft || release.prerelease || !version.pre.is_empty() || version <= current {
        return Ok(None);
    }
    let filename = format!("Anni-{version}-windows-x64-setup.exe");
    let find = |name: &str| -> Result<Asset, String> {
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == name)
            .ok_or("The latest release is not ready for Windows updates yet.")?;
        let expected = format!("{DOWNLOAD_ROOT}{}/{name}", release.tag_name);
        if asset.browser_download_url != expected {
            return Err("The update link is not an official Anni release.".into());
        }
        Ok(asset.clone())
    };
    let installer = find(&filename)?;
    let checksum = find(&format!("{filename}.sha256"))?;
    if installer.size == 0
        || installer.size > MAX_INSTALLER
        || checksum.size == 0
        || checksum.size > 4096
    {
        return Err("The update has an unexpected file size.".into());
    }
    Ok(Some(Available {
        version: version.to_string(),
        installer,
        checksum,
    }))
}
fn checksum(text: &str, filename: &str) -> Result<String, String> {
    let fields: Vec<_> = text.split_whitespace().collect();
    if fields.len() != 2
        || fields[1].trim_start_matches('*') != filename
        || fields[0].len() != 64
        || !fields[0].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("The update checksum is invalid.".into());
    }
    Ok(fields[0].to_ascii_lowercase())
}
async fn read_bounded(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>, String> {
    response
        .error_for_status_ref()
        .map_err(|_| "The update service is unavailable.")?;
    let mut data = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "The update connection was interrupted.")?
    {
        if data.len() + chunk.len() > limit {
            return Err("The update response is too large.".into());
        }
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .https_only(true)
        .user_agent(concat!(
            "Anni/",
            env!("CARGO_PKG_VERSION"),
            " Windows updater"
        ))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(600))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|_| "Could not start the update connection.".into())
}
async fn fetch_and_stage(
    client: &reqwest::Client,
    update: Available,
    root: &Path,
    cancel: &AtomicBool,
    notify: &impl Fn(Progress),
) -> Result<Staged, String> {
    let response = client
        .get(&update.checksum.browser_download_url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| "Could not download the update checksum.")?;
    let bytes = read_bounded(response, 4096).await?;
    let expected = checksum(
        std::str::from_utf8(&bytes).map_err(|_| "The update checksum is unreadable.")?,
        &update.installer.name,
    )?;
    if let Some(digest) = &update.installer.digest {
        if digest != &format!("sha256:{expected}") {
            return Err("The update checksums do not agree.".into());
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Update skipped.".into());
    }
    let directory = root.join(&update.version);
    fs::create_dir_all(&directory).map_err(|_| "Could not create the update folder.")?;
    let destination = directory.join(&update.installer.name);
    let partial = destination.with_extension("exe.part");
    let result = async {
        let mut response = client
            .get(&update.installer.browser_download_url)
            .send()
            .await
            .map_err(|_| "Could not download the update.")?;
        response
            .error_for_status_ref()
            .map_err(|_| "The update download is unavailable.")?;
        if response
            .content_length()
            .is_some_and(|size| size != update.installer.size)
        {
            return Err("The update size does not match the release.".into());
        }
        let mut file = fs::File::create(&partial).map_err(|_| "Could not save the update.")?;
        let mut hash = Sha256::new();
        let mut received = 0;
        let mut last = Instant::now() - Duration::from_secs(1);
        notify(Progress::Downloading {
            version: update.version.clone(),
            received,
            total: update.installer.size,
        });
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "The update download was interrupted.")?
        {
            if cancel.load(Ordering::Relaxed) {
                return Err("Update skipped.".into());
            }
            received += chunk.len() as u64;
            if received > update.installer.size {
                return Err("The update exceeds its expected size.".into());
            }
            file.write_all(&chunk)
                .map_err(|_| "Could not save the update.")?;
            hash.update(&chunk);
            if last.elapsed() >= Duration::from_millis(80) {
                notify(Progress::Downloading {
                    version: update.version.clone(),
                    received,
                    total: update.installer.size,
                });
                last = Instant::now();
            }
        }
        notify(Progress::Verifying {
            version: update.version.clone(),
        });
        if received != update.installer.size || format!("{:x}", hash.finalize()) != expected {
            return Err(
                "The update did not pass verification. Your installed app has not been changed."
                    .into(),
            );
        }
        file.sync_all()
            .map_err(|_| "Could not finish saving the update.")?;
        drop(file);
        // Windows cannot rename over an existing file. Remove only this verified
        // cached installer; never touch anything in the installation directory.
        if destination.exists() {
            fs::remove_file(&destination).map_err(|_| "A previous update is still in use.")?;
        }
        fs::rename(&partial, &destination).map_err(|_| "Could not prepare the verified update.")?;
        Ok(Staged {
            version: update.version,
            installer: destination,
            checksum: expected,
        })
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result
}

pub struct Gate {
    pub progress: Progress,
    rx: mpsc::Receiver<Progress>,
    cancel: Arc<AtomicBool>,
    started: Instant,
    finished: Option<Instant>,
    desktop: bool,
    fullscreen: bool,
    handed_off: bool,
}
pub enum GateAction {
    Wait,
    Continue,
    Close,
}
impl Drop for Gate {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl Gate {
    pub fn start(ctx: &egui::Context, desktop: bool, fullscreen: bool) -> Self {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stopped = cancel.clone();
        let context = ctx.clone();
        std::thread::spawn(move || {
            let notify = |progress| {
                let _ = tx.send(progress);
                context.request_repaint();
            };
            let result = (|| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|_| "Could not start the update checker.")?;
                runtime.block_on(async {
                    let client = client()?;
                    let response = client
                        .get(RELEASES_URL)
                        .header("Accept", "application/vnd.github+json")
                        .timeout(Duration::from_secs(12))
                        .send()
                        .await
                        .map_err(|_| "Could not reach the update service.")?;
                    if response.status() == reqwest::StatusCode::NOT_FOUND {
                        return Ok(Progress::Current);
                    }
                    let bytes = read_bounded(response, 2 * 1024 * 1024).await?;
                    let release = serde_json::from_slice(&bytes)
                        .map_err(|_| "The update service returned an unreadable release.")?;
                    let Some(update) = available(release, env!("CARGO_PKG_VERSION"))? else {
                        return Ok(Progress::Current);
                    };
                    fetch_and_stage(&client, update, &cache_dir(), &stopped, &notify)
                        .await
                        .map(Progress::Staged)
                })
            })();
            notify(result.unwrap_or_else(Progress::Failed));
        });
        Self {
            progress: Progress::Checking,
            rx,
            cancel,
            started: Instant::now(),
            finished: None,
            desktop,
            fullscreen,
            handed_off: false,
        }
    }
    pub fn show(&mut self, ctx: &egui::Context) -> GateAction {
        if self.handed_off {
            return GateAction::Close;
        }
        while let Ok(progress) = self.rx.try_recv() {
            self.progress = progress;
        }
        let skip = crate::ui::updates::show(ctx, &self.progress, self.started.elapsed(), false);
        if skip {
            return GateAction::Continue;
        }
        if let Progress::Staged(staged) = &self.progress {
            match handoff(staged, self.desktop, self.fullscreen) {
                Ok(()) => {
                    self.handed_off = true;
                    return GateAction::Close;
                }
                Err(error) => self.progress = Progress::Failed(error),
            }
        }
        if matches!(self.progress, Progress::Current | Progress::Failed(_)) {
            let finished = self.finished.get_or_insert_with(Instant::now);
            let delay = if matches!(self.progress, Progress::Current) {
                350
            } else {
                2200
            };
            if finished.elapsed() >= Duration::from_millis(delay)
                && self.started.elapsed() >= Duration::from_millis(800)
            {
                return GateAction::Continue;
            }
        }
        ctx.request_repaint_after(Duration::from_millis(16));
        GateAction::Wait
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstallJob {
    parent_pid: u32,
    target: PathBuf,
    installer: PathBuf,
    checksum: String,
    version: String,
    desktop: bool,
    fullscreen: bool,
}
fn handoff(staged: &Staged, desktop: bool, fullscreen: bool) -> Result<(), String> {
    let target = std::env::current_exe().map_err(|_| "Could not locate the installed app.")?;
    if !installed(&target) {
        return Err(
            "Automatic installation is available for installed Windows copies of Anni.".into(),
        );
    }
    let job = InstallJob {
        parent_pid: std::process::id(),
        target: target.clone(),
        installer: staged.installer.clone(),
        checksum: staged.checksum.clone(),
        version: staged.version.clone(),
        desktop,
        fullscreen,
    };
    let path = staged.installer.parent().unwrap().join("install.json");
    fs::write(
        &path,
        serde_json::to_vec(&job).map_err(|_| "Could not prepare the update.")?,
    )
    .map_err(|_| "Could not save the update instructions.")?;
    let helper = cache_dir().join("anni-update.exe");
    // The installed app uses an app-local VC++ runtime. Keep the temporary
    // launcher self-contained on machines without a system-wide VC++ install.
    for dll in ["vcruntime140.dll", "vcruntime140_1.dll"] {
        let source = target.parent().unwrap().join(dll);
        if source.is_file() {
            fs::copy(&source, helper.parent().unwrap().join(dll))
                .map_err(|_| "Could not prepare the update launcher's Windows runtime.")?;
        }
    }
    fs::copy(target, &helper).map_err(|_| "Could not prepare the update launcher.")?;
    hidden_command(&helper)
        .current_dir(helper.parent().unwrap())
        .arg("--apply-update")
        .arg(&path)
        .spawn()
        .map_err(|_| "Could not start the update launcher.")?;
    Ok(())
}
fn hidden_command(exe: &Path) -> Command {
    let mut command = Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command
}
fn validate_job(job: &InstallJob, path: &Path) -> Result<(), String> {
    let version =
        semver::Version::parse(&job.version).map_err(|_| "The update version is invalid.")?;
    if !version.pre.is_empty()
        || job.checksum.len() != 64
        || !job.checksum.bytes().all(|b| b.is_ascii_hexdigit())
        || !installed(&job.target)
    {
        return Err("The update instructions are invalid.".into());
    }
    let root = cache_dir()
        .join(version.to_string())
        .canonicalize()
        .map_err(|_| "The update folder is unavailable.")?;
    let installer = root.join(format!("Anni-{version}-windows-x64-setup.exe"));
    if job
        .installer
        .canonicalize()
        .map_err(|_| "The update installer is missing.")?
        != installer
            .canonicalize()
            .map_err(|_| "The update installer is missing.")?
        || path
            .canonicalize()
            .map_err(|_| "The update instructions are missing.")?
            != root
                .join("install.json")
                .canonicalize()
                .map_err(|_| "The update instructions are missing.")?
    {
        return Err("The update files are outside the expected folder.".into());
    }
    Ok(())
}
fn verify_file(path: &Path, expected: &str) -> Result<(), String> {
    let mut file = fs::File::open(path).map_err(|_| "The downloaded update is missing.")?;
    let size = file
        .metadata()
        .map_err(|_| "Could not inspect the downloaded update.")?
        .len();
    if size == 0 || size > MAX_INSTALLER {
        return Err("The downloaded update has an invalid size.".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| "Could not verify the downloaded update.")?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    if format!("{:x}", hash.finalize()) != expected {
        return Err("Update verification failed. The installer was not run.".into());
    }
    Ok(())
}
#[cfg(windows)]
fn wait_for_parent(job: &InstallJob) -> Result<(), String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_INVALID_PARAMETER, WAIT_OBJECT_0},
        System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject,
            PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        },
    };
    unsafe {
        let process = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            job.parent_pid,
        );
        if process.is_null() {
            return if GetLastError() == ERROR_INVALID_PARAMETER {
                Ok(())
            } else {
                Err("Could not wait for Anni to close.".into())
            };
        }
        let mut buffer = [0_u16; 32768];
        let mut length = buffer.len() as u32;
        let queried = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) != 0;
        let image = PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize]));
        let same = queried
            && image
                .to_string_lossy()
                .eq_ignore_ascii_case(&job.target.to_string_lossy());
        let waited = same && WaitForSingleObject(process, 30_000) == WAIT_OBJECT_0;
        CloseHandle(process);
        if waited {
            Ok(())
        } else {
            Err("Anni is still open. The update will be retried next time.".into())
        }
    }
}
#[cfg(not(windows))]
fn wait_for_parent(_: &InstallJob) -> Result<(), String> {
    Err("Automatic installation is supported on Windows.".into())
}
fn installer_command(job: &InstallJob) -> Command {
    let mut command = hidden_command(&job.installer);
    command
        .args([
            "/SP-",
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/CLOSEAPPLICATIONS",
            "/NORESTARTAPPLICATIONS",
            "/AUTOMATICUPDATE=1",
        ])
        .arg(format!("/DIR={}", job.target.parent().unwrap().display()))
        .arg(format!(
            "/LOG={}",
            job.installer
                .parent()
                .unwrap()
                .join("install.log")
                .display()
        ));
    command
}
fn launch(job: &InstallJob) -> Result<(), String> {
    if !installed(&job.target) {
        return Err(
            "Anni is unavailable at its installed location. Please run setup again.".into(),
        );
    }
    let mut command = hidden_command(&job.target);
    command.arg("--skip-update");
    if job.desktop {
        command.arg("--desktop");
    }
    if job.fullscreen {
        command.arg("--fullscreen");
    }
    command.spawn().map(|_| ()).map_err(|_| {
        "The update finished, but Anni could not reopen. Open it from your shortcut.".into()
    })
}
fn apply(job: &InstallJob) -> Result<(), String> {
    wait_for_parent(job)?;
    #[cfg(windows)]
    let instance = crate::desktop_platform::single_instance()
        .ok_or("Anni was reopened before the update could install. Close it and try again.")?;
    verify_file(&job.installer, &job.checksum)?;
    let status = installer_command(job)
        .status()
        .map_err(|_| "The verified update installer could not start.")?;
    if !status.success() {
        return Err("The update could not finish. Your saved schoolwork and settings are still on this computer.".into());
    }
    #[cfg(windows)]
    drop(instance);
    launch(job)?;
    let _ = fs::remove_file(&job.installer);
    Ok(())
}

pub fn viewport() -> egui::ViewportBuilder {
    let area = crate::desktop_platform::work_area();
    let size = egui::vec2(560.0, 318.0);
    egui::ViewportBuilder::default()
        .with_title("anni")
        .with_app_id("anni-updater")
        .with_icon(crate::ui::brand::icon())
        .with_inner_size(size)
        .with_min_inner_size(size)
        .with_position(area.center() - size * 0.5)
        .with_resizable(false)
        .with_decorations(false)
        .with_transparent(true)
}
pub fn run_apply(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|_| "Could not read the update instructions.")?;
    if bytes.len() > 16 * 1024 {
        return Err("The update instructions are invalid.".into());
    }
    let job: InstallJob =
        serde_json::from_slice(&bytes).map_err(|_| "The update instructions are unreadable.")?;
    let validation = validate_job(&job, path);
    struct ApplyApp {
        rx: mpsc::Receiver<Result<(), String>>,
        job: InstallJob,
        progress: Progress,
        started: Instant,
        done: Option<Instant>,
    }
    impl eframe::App for ApplyApp {
        fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
            [0.0; 4]
        }
        fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
            if let Ok(result) = self.rx.try_recv() {
                self.progress = result
                    .map(|_| Progress::Complete)
                    .unwrap_or_else(Progress::Failed);
                self.done = Some(Instant::now());
            }
            if ctx.input(|i| i.viewport().close_requested()) && self.done.is_none() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
            if crate::ui::updates::show(ctx, &self.progress, self.started.elapsed(), true) {
                match launch(&self.job) {
                    Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    Err(error) => self.progress = Progress::Failed(error),
                }
            }
            if matches!(self.progress, Progress::Complete)
                && self
                    .done
                    .is_some_and(|done| done.elapsed() > Duration::from_millis(300))
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
    let worker = job.clone();
    eframe::run_native(
        "anni",
        eframe::NativeOptions {
            viewport: viewport(),
            persist_window: false,
            ..Default::default()
        },
        Box::new(move |cc| {
            crate::ui::theme::apply_theme(&cc.egui_ctx);
            let (tx, rx) = mpsc::channel();
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(validation.and_then(|_| apply(&worker)));
                ctx.request_repaint();
            });
            Ok(Box::new(ApplyApp {
                rx,
                progress: Progress::Installing {
                    version: job.version.clone(),
                },
                job,
                started: Instant::now(),
                done: None,
            }))
        }),
    )
    .map_err(|error| format!("Could not open the update window: {error}"))
}

/// No network, downloads, saved preferences, or installer execution in preview mode.
pub fn preview() {
    struct Preview(Instant);
    impl eframe::App for Preview {
        fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
            [0.0; 4]
        }
        fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
            let elapsed = self.0.elapsed();
            let second = elapsed.as_secs_f32() % 18.0;
            let progress = if second < 4.0 {
                Progress::Checking
            } else if second < 12.0 {
                Progress::Downloading {
                    version: "0.2.1".into(),
                    received: (((second - 4.0) / 8.0) * 26_214_400.0) as u64,
                    total: 26_214_400,
                }
            } else if second < 14.0 {
                Progress::Verifying {
                    version: "0.2.1".into(),
                }
            } else if second < 17.0 {
                Progress::Installing {
                    version: "0.2.1".into(),
                }
            } else {
                Progress::Complete
            };
            if crate::ui::updates::show(ctx, &progress, elapsed, true) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
    eframe::run_native(
        "anni updater preview",
        eframe::NativeOptions {
            viewport: viewport()
                .with_title("Anni · update preview")
                .with_app_id("anni-update-preview"),
            persist_window: false,
            ..Default::default()
        },
        Box::new(|cc| {
            crate::ui::theme::apply_theme(&cc.egui_ctx);
            cc.egui_ctx
                .data_mut(|data| data.insert_temp(egui::Id::new("update_preview"), true));
            Ok(Box::new(Preview(Instant::now())))
        }),
    )
    .expect("could not open update preview");
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(version: &str) -> Release {
        let name = format!("Anni-{version}-windows-x64-setup.exe");
        serde_json::from_value(serde_json::json!({"tag_name":format!("v{version}"), "assets":[
            {"name":name,"browser_download_url":format!("{DOWNLOAD_ROOT}v{version}/{name}"),"size":100},
            {"name":format!("{name}.sha256"),"browser_download_url":format!("{DOWNLOAD_ROOT}v{version}/{name}.sha256"),"size":110}
        ]})).unwrap()
    }
    fn directory(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "anni-update-{label}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
    #[test]
    fn versions_compare_numerically_and_never_downgrade_or_install_prereleases() {
        assert!(available(release("0.10.0"), "0.9.0").unwrap().is_some());
        assert!(available(release("0.9.0"), "0.10.0").unwrap().is_none());
        assert!(available(release("0.2.0"), "0.2.0").unwrap().is_none());
        assert!(available(release("1.0.0-beta.1"), "0.2.0")
            .unwrap()
            .is_none());
        let mut draft = release("1.0.0");
        draft.draft = true;
        assert!(available(draft, "0.2.0").unwrap().is_none());
    }
    #[test]
    fn release_requires_matching_official_windows_assets() {
        let mut wrong = release("1.0.0");
        wrong.assets[0].browser_download_url = "https://example.org/installer.exe".into();
        assert!(available(wrong, "0.2.0").is_err());
        let mut missing = release("1.0.0");
        missing.assets.pop();
        assert!(available(missing, "0.2.0").is_err());
        let mut oversized = release("1.0.0");
        oversized.assets[0].size = MAX_INSTALLER + 1;
        assert!(available(oversized, "0.2.0").is_err());
    }
    #[test]
    fn checksums_are_bound_to_the_exact_installer() {
        let hash = "a".repeat(64);
        assert_eq!(
            checksum(
                &format!("{hash}  Anni-1.0.0-windows-x64-setup.exe\r\n"),
                "Anni-1.0.0-windows-x64-setup.exe"
            )
            .unwrap(),
            hash
        );
        assert!(checksum(
            &format!("{hash}  different.exe"),
            "Anni-1.0.0-windows-x64-setup.exe"
        )
        .is_err());
        assert!(checksum("not-a-hash update.exe", "update.exe").is_err());
    }
    #[test]
    fn modified_staged_installers_fail_the_second_verification() {
        let dir = directory("hash");
        let path = dir.join("update.exe");
        fs::write(&path, b"verified installer").unwrap();
        let hash = format!("{:x}", Sha256::digest(b"verified installer"));
        assert!(verify_file(&path, &hash).is_ok());
        fs::write(&path, b"changed installer").unwrap();
        assert!(verify_file(&path, &hash).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn installer_arguments_preserve_install_path_and_never_change_startup_preference() {
        let job = InstallJob {
            parent_pid: 0,
            target: PathBuf::from("C:/Anni with spaces/anni.exe"),
            installer: PathBuf::from("C:/Update folder/update.exe"),
            checksum: "a".repeat(64),
            version: "1.0.0".into(),
            desktop: true,
            fullscreen: false,
        };
        let command = installer_command(&job);
        let args: Vec<_> = command
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"/AUTOMATICUPDATE=1".into()));
        assert!(args.contains(&"/DIR=C:/Anni with spaces".into()));
        assert!(args.contains(&"/NORESTART".into()));
        assert!(!args
            .iter()
            .any(|arg| arg.starts_with("/TASKS=") || arg.contains("FORCE")));
    }
    #[test]
    fn source_builds_and_mock_mode_do_not_start_automatic_updates() {
        assert!(!should_check(true, false));
        assert!(!should_check(false, true));
        assert!(!installed(Path::new("target/release/anni.exe")));
    }
    #[tokio::test]
    async fn download_is_staged_only_after_complete_size_and_checksum_verification() {
        use std::net::TcpListener;
        let body = b"fixture installer bytes";
        for valid in [true, false] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let hash = format!(
                "{:x}",
                Sha256::digest(if valid {
                    body.as_slice()
                } else {
                    b"wrong bytes"
                })
            );
            let name = "Anni-1.0.0-windows-x64-setup.exe";
            let text = format!("{hash}  {name}\n");
            let server = std::thread::spawn(move || {
                for content in [text.as_bytes(), body.as_slice()] {
                    let (mut stream, _) = listener.accept().unwrap();
                    let mut request = [0; 8192];
                    stream.read(&mut request).unwrap();
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        content.len()
                    )
                    .unwrap();
                    stream.write_all(content).unwrap();
                }
            });
            let root = directory("download");
            let update = Available {
                version: "1.0.0".into(),
                installer: Asset {
                    name: name.into(),
                    browser_download_url: format!("http://{address}/installer"),
                    size: body.len() as u64,
                    digest: None,
                },
                checksum: Asset {
                    name: format!("{name}.sha256"),
                    browser_download_url: format!("http://{address}/checksum"),
                    size: 110,
                    digest: None,
                },
            };
            let client = reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap();
            let result =
                fetch_and_stage(&client, update, &root, &AtomicBool::new(false), &|_| {}).await;
            assert_eq!(result.is_ok(), valid);
            assert_eq!(root.join("1.0.0").join(name).exists(), valid);
            assert!(!root
                .join("1.0.0")
                .join(name)
                .with_extension("exe.part")
                .exists());
            server.join().unwrap();
            fs::remove_dir_all(root).unwrap();
        }
    }
}
