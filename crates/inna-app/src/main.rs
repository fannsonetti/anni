#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[macro_use]
mod locale;
mod app;
mod class_names;
mod day;
mod desktop;
mod desktop_platform;
mod diagnostics;
mod grades;
mod mock;
mod persistence;
mod phone_auth;
mod planner;
mod runtime;
mod settings;
mod state;
#[cfg(windows)]
mod tray;
mod ui;
mod updates;

use clap::Parser;

#[derive(Parser)]
#[command(name = "anni", version, about = "anni — your school portal for INNA")]
struct Cli {
    /// Use sample data without connecting to INNA
    #[arg(long)]
    mock: bool,

    /// Open fullscreen
    #[arg(long)]
    fullscreen: bool,

    /// Show desktop homework and clock without the planner window
    #[arg(long, conflicts_with = "fullscreen")]
    desktop: bool,

    /// Test session validity and exit
    #[arg(long)]
    test_session: bool,

    /// Internal: the update helper reopens the app without checking a second time
    #[arg(long, hide = true)]
    skip_update: bool,

    /// Internal: apply a verified staged update from a separate copy of Anni
    #[arg(long, hide = true)]
    apply_update: Option<std::path::PathBuf>,

    /// Preview the startup update animation without connecting or installing
    #[arg(long)]
    preview_update: bool,

    /// Receive anonymous bug reports on TCP port 3000 on this computer
    #[arg(long)]
    diagnostics_server: bool,
}

fn main() {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter("anni=info,inna_core=info,inna_cache=info")
        .init();

    tracing::info!("starting anni");

    // The update screen, tray menu, and planner all use the saved app language.
    if !cli.mock {
        if let Ok(settings) = settings::DesktopSettings::load() {
            locale::set_language(settings.language);
        }
    }

    if let Some(path) = &cli.apply_update {
        if let Err(error) = updates::run_apply(path) {
            tracing::error!("Update launcher failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if cli.preview_update {
        updates::preview();
        return;
    }

    if cli.test_session {
        if let Err(error) = verify_schoolwork() {
            tracing::error!("INNA check failed: {error}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(windows)]
    let _instance = if !cli.mock {
        match desktop_platform::single_instance() {
            Some(instance) => Some(instance),
            None => return,
        }
    } else {
        None
    };

    diagnostics::init(cli.mock, cli.diagnostics_server);

    let check_updates = updates::should_check(cli.mock, cli.skip_update);
    let options = eframe::NativeOptions {
        // Native auto-save also runs on child widgets. Anni stores root bounds itself.
        persist_window: false,
        viewport: if check_updates {
            updates::viewport().with_app_id("anni")
        } else {
            egui::ViewportBuilder::default()
                .with_title(ui::brand::NAME)
                .with_app_id(if cli.mock { "anni-preview" } else { "anni" })
                .with_icon(ui::brand::icon())
                .with_taskbar(false)
                // All viewports share the root's GL configuration, including its alpha channel.
                .with_transparent(true)
                .with_inner_size([1280.0, 860.0])
                .with_min_inner_size([800.0, 480.0])
                .with_fullscreen(cli.fullscreen)
        },
        ..Default::default()
    };

    let mock_mode = cli.mock;
    // Anni lives in the notification area instead of the taskbar.
    #[cfg(windows)]
    let _tray = tray::create();

    eframe::run_native(
        ui::brand::NAME,
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::AnniApp::new_with_updates(
                cc,
                mock_mode,
                cli.desktop,
                check_updates,
                cli.fullscreen,
            )))
        }),
    )
    .expect("failed to start eframe");
}

/// Read-only diagnostics for the same importer used by the planner; never saves progress.
fn verify_schoolwork() -> Result<(), Box<dyn std::error::Error>> {
    use inna_core::session::{FileSessionStore, SessionStore};
    let session = FileSessionStore::new(FileSessionStore::default_path())
        .load()?
        .ok_or("No saved INNA session. Connect in Anni first.")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let client = inna_core::client::InnaClient::new()?;
        client.load_session(&session);
        let user = client.current_user().await?;
        let today = chrono::Utc::now().date_naive();
        let timetable = client.timetable(user.student_id_str().ok_or("INNA did not return a student ID")?,
            today, today + chrono::Duration::days(13)).await?;
        tracing::info!("Timetable verified: {} active lessons, {} teacher-absence cancellations",
            timetable.iter().filter(|entry| entry.is_active_lesson()).count(),
            timetable.iter().filter(|entry| entry.teacher_absent).count());
        let work = client.schoolwork(vec![], vec![], &timetable, today).await?;
        let courses = work.assignments.len();
        let mut state = state::AppState::new();
        state.assignments = work.assignments;
        state.homework = work.homework;
        let items = planner::work_items(&state);
        let homework = items.iter().filter(|i| i.kind == planner::WorkKind::Homework).count();
        let assignments = items.iter().filter(|i| i.kind == planner::WorkKind::Assignment && !i.done).count();
        tracing::info!("INNA verified: {courses} courses, {homework} Heimavinna items, {assignments} unfinished course assignments");
        for (_, warning) in &work.warnings { tracing::warn!("{warning}"); }
        if !work.warnings.is_empty() { return Err("Some INNA sources could not be checked".into()); }
        Ok(())
    })
}
