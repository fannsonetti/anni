#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod desktop;
mod desktop_platform;
mod focus;
mod grades;
mod locale;
mod mock;
mod persistence;
mod phone_auth;
mod planner;
mod runtime;
mod settings;
mod state;
mod ui;

use clap::Parser;

#[derive(Parser)]
#[command(name = "anni", about = "anni — your school planner for INNA")]
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
}

fn main() {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter("anni=info,inna_core=info,inna_cache=info")
        .init();

    tracing::info!("starting anni");

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

    let options = eframe::NativeOptions {
        // Native auto-save also runs on child widgets. Anni stores root bounds itself.
        persist_window: false,
        viewport: egui::ViewportBuilder::default()
            .with_title(ui::brand::NAME)
            .with_app_id(if cli.mock { "anni-preview" } else { "anni" })
            .with_icon(ui::brand::icon())
            // All viewports share the root's GL configuration, including its alpha channel.
            .with_transparent(true)
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([800.0, 480.0])
            .with_fullscreen(cli.fullscreen),
        ..Default::default()
    };

    let mock_mode = cli.mock;

    eframe::run_native(
        ui::brand::NAME,
        options,
        Box::new(move |cc| Ok(Box::new(app::AnniApp::new(cc, mock_mode, cli.desktop)))),
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
        client.current_user().await?;
        let work = client.schoolwork(vec![], vec![], &[], chrono::Utc::now().date_naive()).await?;
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
