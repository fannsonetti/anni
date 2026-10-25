use crate::mock;
use crate::persistence;
use crate::runtime::{Runtime, RuntimeMsg};
use crate::state::{AppState, ConnectionState, Page};
use crate::ui::{self, theme};
use chrono::Utc;
use egui::RichText;
use inna_cache::database::Database;
use inna_core::session::{FileSessionStore, SessionStore};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

pub struct AnniApp {
    shared: Arc<Mutex<AppData>>,
    planner_rect: egui::Rect,
    restore_window: bool,
    update_gate: Option<crate::updates::Gate>,
    fullscreen: bool,
}

impl AnniApp {
    #[cfg(test)]
    pub fn new(cc: &eframe::CreationContext<'_>, mock_mode: bool, desktop_only: bool) -> Self {
        Self::new_with_updates(cc, mock_mode, desktop_only, false, false)
    }

    pub fn new_with_updates(
        cc: &eframe::CreationContext<'_>,
        mock_mode: bool,
        desktop_only: bool,
        check_updates: bool,
        fullscreen: bool,
    ) -> Self {
        let area = crate::desktop_platform::work_area();
        let default_rect = egui::Rect::from_center_size(
            area.center(),
            egui::vec2(1280.0_f32.min(area.width()), 860.0_f32.min(area.height())),
        );
        let planner_rect: egui::Rect = if mock_mode {
            default_rect
        } else {
            cc.storage
                .and_then(|storage| eframe::get_value(storage, "planner_window_v1"))
                .unwrap_or(default_rect)
        };
        let planner_rect = if planner_rect.is_finite()
            && planner_rect.width() >= 800.0
            && planner_rect.height() >= 480.0
            && planner_rect.intersects(area)
        {
            planner_rect
        } else {
            default_rect
        };
        Self {
            shared: Arc::new(Mutex::new(AppData::new(cc, mock_mode, desktop_only))),
            planner_rect,
            restore_window: true,
            update_gate: check_updates
                .then(|| crate::updates::Gate::start(&cc.egui_ctx, desktop_only, fullscreen)),
            fullscreen,
        }
    }
}

pub struct AppData {
    pub desktop: crate::desktop::Desktop,
    pub state: AppState,
    pub mock_mode: bool,
    runtime: Option<Runtime>,
    rx: mpsc::Receiver<RuntimeMsg>,
    database: Option<Database>,
    last_ping: Option<Instant>,
    last_sync: Option<Instant>,
    auth_in_progress: bool,
    phone_auth_url: Option<String>,
    browser_auth_url: Option<String>,
    last_diagnostic: Option<Instant>,
}

impl AppData {
    pub fn new(cc: &eframe::CreationContext<'_>, mock_mode: bool, desktop_only: bool) -> Self {
        theme::apply_theme(&cc.egui_ctx);
        let mut state = AppState::new();
        let (tx, rx) = mpsc::channel();
        let database = if mock_mode {
            state.connection = ConnectionState::MockMode;
            state.user = Some(mock::mock_user());
            state.timetable = mock::mock_timetable();
            state.announcements = mock::mock_announcements();
            state.assignments = mock::mock_assignments();
            state.homework = mock::mock_homework();
            state.todo_items = mock::mock_todo();
            for resource in [
                "timetable",
                "announcements",
                "assignments",
                "homework",
                "todo",
            ] {
                state
                    .last_sync
                    .insert(resource.into(), Utc::now().to_rfc3339());
            }
            None
        } else {
            match Database::open(&Database::default_path()) {
                Ok(db) => {
                    if let Err(error) = persistence::restore(&db, &mut state) {
                        tracing::warn!("Could not restore saved data: {error}");
                        state.storage_error = Some("Could not load saved data.".into());
                    }
                    Some(db)
                }
                Err(error) => {
                    tracing::warn!("Local storage unavailable: {error}");
                    state.storage_error = Some(
                        "Storage is unavailable. Changes will be lost if you close Anni.".into(),
                    );
                    None
                }
            }
        };
        let runtime = if !mock_mode {
            let rt = Runtime::new(tx);
            rt.start_session_check(cc.egui_ctx.clone());
            Some(rt)
        } else {
            None
        };
        Self {
            desktop: crate::desktop::Desktop::new(mock_mode, desktop_only),
            state,
            mock_mode,
            runtime,
            rx,
            database,
            last_ping: None,
            last_sync: None,
            auth_in_progress: false,
            phone_auth_url: None,
            browser_auth_url: None,
            last_diagnostic: None,
        }
    }

    fn synced(&mut self, resource: &str) {
        self.state
            .last_sync
            .insert(resource.into(), Utc::now().to_rfc3339());
        self.state.sync_errors.remove(resource);
        self.state.storage_dirty = true;
    }

    fn tick_focus(&mut self, now: Instant) {
        if self.state.focus.tick(now) && !self.state.focus.is_break {
            self.state
                .planner
                .focus_sessions
                .push(crate::planner::FocusSession {
                    date: Utc::now().date_naive(),
                    minutes: self.state.focus.minutes,
                    task: self.state.focus.task_title.clone(),
                });
            self.state.storage_dirty = true;
        }
    }

    fn request_refresh(&mut self, ctx: &egui::Context) {
        self.state.refresh_requested = false;
        if self.state.refreshing || self.mock_mode || self.auth_in_progress {
            return;
        }
        if let (Some(rt), Some(user)) = (&self.runtime, &self.state.user) {
            if matches!(
                self.state.connection,
                ConnectionState::Connected | ConnectionState::Offline
            ) {
                self.state.refreshing = true;
                self.state.sync_errors.remove("connection");
                rt.start_sync(
                    user.student_id.clone(),
                    self.state.assignments.clone(),
                    self.state.homework.clone(),
                    self.state.timetable.clone(),
                    ctx.clone(),
                );
                self.last_sync = Some(Instant::now());
            }
        }
    }

    fn process_messages(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                RuntimeMsg::SessionLoaded => {
                    self.state.connection = ConnectionState::Authenticating
                }
                RuntimeMsg::Connected(user) => {
                    crate::diagnostics::record(crate::diagnostics::Event::Connected);
                    self.state.adopt_user(user);
                    self.state.connection = ConnectionState::Connected;
                    self.auth_in_progress = false;
                    self.phone_auth_url = None;
                    self.browser_auth_url = None;
                    self.state.storage_dirty = true;
                    self.request_refresh(ctx);
                }
                RuntimeMsg::SessionExpired => {
                    crate::diagnostics::failure(crate::diagnostics::Resource::Connection, crate::diagnostics::Failure::SessionExpired);
                    self.state.connection = ConnectionState::SessionExpired;
                    self.auth_in_progress = false;
                    self.phone_auth_url = None;
                    self.browser_auth_url = None;
                }
                RuntimeMsg::AuthNeeded => self.state.connection = ConnectionState::Starting,
                RuntimeMsg::AuthStarted => {
                    self.state.navigate(Page::Connection);
                    self.auth_in_progress = true;
                    self.phone_auth_url = None;
                    self.browser_auth_url = None;
                    self.state.connection = ConnectionState::Authenticating;
                }
                RuntimeMsg::PhoneAuthReady(url) => {
                    self.phone_auth_url = Some(url);
                    self.auth_in_progress = true;
                }
                RuntimeMsg::BrowserAuthReady(url) => {
                    if self.auth_in_progress { self.browser_auth_url = Some(url); }
                }
                RuntimeMsg::AuthFailed(error) => {
                    crate::diagnostics::failure(crate::diagnostics::Resource::Connection, crate::diagnostics::Failure::classify(&error));
                    self.auth_in_progress = false;
                    self.phone_auth_url = None;
                    self.browser_auth_url = None;
                    self.state.connection = ConnectionState::Error(error);
                }
                RuntimeMsg::TimetableUpdated(data) => {
                    self.state.timetable = data;
                    self.synced("timetable");
                }
                RuntimeMsg::AnnouncementsUpdated(data) => {
                    self.state.announcements = data;
                    self.state.announcements_replaced();
                    self.synced("announcements");
                }
                RuntimeMsg::AssignmentsUpdated(data) => {
                    self.state.assignments = data;
                    self.synced("assignments");
                }
                RuntimeMsg::HomeworkUpdated(data) => {
                    self.state.homework = data;
                    self.synced("homework");
                }
                RuntimeMsg::TodoUpdated(data) => {
                    self.state.todo_items = data;
                    self.synced("todo");
                }
                RuntimeMsg::SyncError(resource, error) => {
                    crate::diagnostics::failure(crate::diagnostics::Resource::from_name(&resource), crate::diagnostics::Failure::classify(&error));
                    tracing::warn!("{resource} sync failed: {error}");
                    self.state.sync_errors.insert(resource, error);
                }
                RuntimeMsg::SyncFinished => {
                    self.state.refreshing = false;
                    if !self.mock_mode && self.last_diagnostic.is_none_or(|last| last.elapsed().as_secs() >= 3600) {
                        crate::diagnostics::record(crate::diagnostics::Event::SyncSummary {
                            lessons: self.state.timetable.iter().filter(|entry| entry.is_lesson()).count(),
                            courses: self.state.assignments.len(), homework: self.state.homework.len(),
                            warnings: self.state.sync_errors.len(),
                            generic_course_labels: self.state.timetable.iter().filter(|entry| entry.is_lesson()
                                && ui::assignments::subject(entry.course_code()) == entry.course_code()).count(),
                            non_lesson_entries: self.state.timetable.iter().filter(|entry| !entry.is_lesson()).count(),
                            invalid_lesson_times: self.state.timetable.iter().filter(|entry| entry.is_lesson()
                                && (entry.start.as_deref().and_then(crate::planner::parse_deadline).is_none()
                                    || entry.end.as_deref().and_then(crate::planner::parse_deadline).is_none())).count(),
                        });
                        self.last_diagnostic = Some(Instant::now());
                    }
                },
                RuntimeMsg::PingOk => {
                    if self.state.connection == ConnectionState::Offline {
                        self.state.connection = ConnectionState::Connected;
                        self.state.refresh_requested = true;
                    }
                }
                RuntimeMsg::Offline => {
                    crate::diagnostics::failure(crate::diagnostics::Resource::Connection, crate::diagnostics::Failure::Network);
                    self.state.connection = ConnectionState::Offline;
                },
            }
        }
    }

    fn periodic_tasks(&mut self, ctx: &egui::Context) {
        if self.mock_mode
            || !matches!(
                self.state.connection,
                ConnectionState::Connected | ConnectionState::Offline
            )
        {
            return;
        }
        let now = Instant::now();
        if self
            .last_ping
            .is_none_or(|t| now.duration_since(t).as_secs() >= 60)
        {
            if let Some(rt) = &self.runtime {
                if self.state.user.is_some() {
                    rt.start_ping(ctx.clone());
                } else {
                    rt.start_session_check(ctx.clone());
                }
                self.last_ping = Some(now);
            }
        }
        if self
            .last_sync
            .is_some_and(|t| now.duration_since(t).as_secs() >= 300)
        {
            self.request_refresh(ctx);
        }
    }

    fn save_changes(&mut self) {
        if !self.state.storage_dirty {
            return;
        }
        self.state.storage_dirty = false;
        if let Some(db) = &self.database {
            match persistence::save(db, &self.state) {
                Ok(()) => self.state.storage_error = None,
                Err(error) => {
                    tracing::warn!("Could not save planner: {error}");
                    crate::diagnostics::failure(crate::diagnostics::Resource::Storage, crate::diagnostics::Failure::Storage);
                    self.state.storage_error =
                        Some("Could not save changes. Keep Anni open and try saving again.".into())
                }
            }
        }
    }

    fn connection_action(&mut self, action: ui::connection::ConnectionAction, ctx: &egui::Context) {
        use ui::connection::ConnectionAction;
        match action {
            ConnectionAction::ReconnectBrowser if !self.auth_in_progress && !self.state.refreshing => {
                if let Some(rt) = &self.runtime { rt.launch_browser_auth(ctx.clone()); }
            }
            ConnectionAction::ReconnectLocal | ConnectionAction::ReconnectPhone
                if !self.auth_in_progress && !self.state.refreshing =>
            {
                if let Some(rt) = &self.runtime {
                    rt.launch_auth(ctx.clone(), action == ConnectionAction::ReconnectPhone);
                }
            }
            ConnectionAction::Disconnect => {
                let store = FileSessionStore::new(FileSessionStore::default_path());
                if self.mock_mode || store.clear().is_ok() {
                    self.state.connection = ConnectionState::SessionExpired;
                } else {
                    self.state.connection = ConnectionState::Error(
                        "Could not remove your saved sign-in. Please try again.".into(),
                    );
                }
            }
            _ => {}
        }
    }
}

impl eframe::App for AnniApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if !self.shared.lock().unwrap().mock_mode {
            // eframe's automatic native-window save can run for a desktop viewport.
            // Save the planner bounds observed in the root viewport instead.
            eframe::set_value(storage, "planner_window_v1", &self.planner_rect);
        }
    }

    fn persist_egui_memory(&self) -> bool {
        !self.shared.lock().unwrap().mock_mode
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(gate) = &mut self.update_gate {
            match gate.show(ctx) {
                crate::updates::GateAction::Wait => return,
                crate::updates::GateAction::Close => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
                crate::updates::GateAction::Continue => {
                    self.update_gate = None;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
                }
            }
        }
        if self.restore_window {
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
                800.0, 480.0,
            )));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(self.planner_rect.size()));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(self.planner_rect.min));
            self.restore_window = false;
            ctx.request_repaint();
        } else if !self.shared.lock().unwrap().desktop.quit_requested {
            ctx.input(|i| {
                if let Some(inner) = i.viewport().inner_rect {
                    if inner.width() >= 800.0 && inner.height() >= 480.0 {
                        self.planner_rect = egui::Rect::from_min_size(
                            i.viewport().outer_rect.unwrap_or(inner).min,
                            inner.size(),
                        );
                    }
                }
            });
        }
        self.shared.lock().unwrap().planner_ui(ctx);
        ui::desktop::show(ctx, self.shared.clone());
    }
}

impl AppData {
    /// Visible desktop viewports keep synchronization and persistence alive when
    /// Windows stops sending paints to the hidden planner window.
    pub fn service(&mut self, ctx: &egui::Context) {
        self.tick_focus(Instant::now());
        self.process_messages(ctx);
        self.periodic_tasks(ctx);
    }

    pub fn finish_update(&mut self, ctx: &egui::Context) {
        self.desktop.tick(ctx, self.mock_mode);
        if self.state.refresh_requested {
            self.request_refresh(ctx);
        }
        if self.state.storage_dirty {
            ctx.request_repaint_of(egui::ViewportId::ROOT);
            for id in crate::desktop::widget_ids() {
                ctx.request_repaint_of(id);
            }
        }
        self.save_changes();
    }

    fn planner_ui(&mut self, ctx: &egui::Context) {
        if !self.desktop.quit_requested
            && ctx.input(|i| {
                i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::WindowFocused(true)))
            })
        {
            // A second launch can reveal the root through the native instance handler.
            self.desktop.planner_hidden = false;
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.desktop.quit_requested {
            self.desktop.handle(
                crate::desktop::DesktopAction::Quit,
                ctx,
                &mut self.state,
            );
        }
        ctx.set_theme(egui::ThemePreference::Dark);
        self.service(ctx);
        if self.desktop.quit_requested {
            self.finish_update(ctx);
            return;
        }
        if self.desktop.planner_hidden {
            self.finish_update(ctx);
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(15));
        if self.state.focus.running() {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        ui::navigation::shortcuts(ctx, &mut self.state);
        let wide = ctx.screen_rect().width() >= 1040.0;
        if wide {
            ui::navigation::sidebar(ctx, &mut self.state);
        }

        egui::TopBottomPanel::top("status_bar")
            .frame(egui::Frame::none().fill(theme::NAV_BG).inner_margin(egui::Margin::symmetric(24.0, 12.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if !wide && ui::brand::wordmark(ui, true).clicked() { self.state.navigate(Page::Home); }
                    ui::navigation::location(ui, &mut self.state);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(theme::primary_button("+ New task")).clicked() { self.state.new_task(); }
                        let can_refresh = !self.mock_mode && !self.state.refreshing && !self.auth_in_progress
                            && matches!(self.state.connection, ConnectionState::Connected | ConnectionState::Offline) && self.state.user.is_some();
                        if ui.add_enabled(can_refresh, egui::Button::new(if self.state.refreshing { "Refreshing…" } else { "Refresh" }).frame(false)).clicked() { self.state.refresh_requested = true; }
                        if self.state.focus.in_progress() || self.state.focus.completed {
                            if ui.button(format!("{} · Focus", self.state.focus.clock(Instant::now()))).clicked() { self.state.navigate(Page::Focus); }
                        }
                        if ctx.screen_rect().width() >= 1150.0 {
                            ui.label(RichText::new(self.state.connection.label()).size(11.0).color(theme::TEXT_SECONDARY));
                            let (r, _) = ui.allocate_exact_size(egui::vec2(6.0,6.0),egui::Sense::hover());
                            ui.painter().circle_filled(r.center(),3.0,self.state.connection.color());
                        }
                    });
                });
            if self.mock_mode {ui.label(RichText::new("Demo data · changes are temporary").size(12.0).color(theme::WARNING));}
            if self.auth_in_progress && ui.small_button("Sign-in details").clicked(){self.state.navigate(Page::Connection);}
            if !self.mock_mode && !matches!(self.state.connection,ConnectionState::Connected){
                ui.horizontal_wrapped(|ui| {
                    let text=match &self.state.connection {
                        ConnectionState::Offline=>"You're offline. Your saved plan is available; Anni will reconnect automatically.",
                        ConnectionState::Starting=>"Connect INNA to get your timetable, assignments, and grades.",
                        ConnectionState::SessionExpired=>"Reconnect to INNA to update your saved data.",
                        ConnectionState::Error(_)=>"Could not connect to INNA. Please try reconnecting.",
                        _=>"Checking your INNA connection…",
                    };
                    ui.label(RichText::new(text).size(13.0).color(theme::WARNING));
                    if !self.auth_in_progress && ui.small_button("Connection settings").clicked(){self.state.navigate(Page::Connection);}
                });
            }
            if !self.state.sync_errors.is_empty(){
                let mut resources:Vec<_>=self.state.sync_errors.keys().map(|s|crate::locale::resource(s)).collect();resources.sort();
                ui.label(RichText::new(format!("Could not update {}. Showing your last saved data.",resources.join(", "))).size(13.0).color(theme::WARNING));
            }
            if let Some(error)=self.state.storage_error.clone(){
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(theme::ERROR_RED,error);
                    if self.database.is_some() && ui.small_button("Retry saving").clicked(){self.state.storage_dirty=true;}
                });
            }
        });
        if !wide {
            ui::navigation::compact_bar(ctx, &mut self.state);
        }
        egui::CentralPanel::default().frame(egui::Frame::none().fill(theme::BG_DARK).inner_margin(if wide { 26.0 } else { 18.0 })).show(ctx,|ui| {
            if self.auth_in_progress && self.state.selected_page==Page::Connection {
                theme::card().show(ui,|ui| {
                    if let Some(url)=&self.browser_auth_url {
                        theme::page_title(ui,"Connect using your browser","Use the browser profile where you're already signed in to Google or INNA.");
                        ui.hyperlink_to("Open browser connection page",url);
                        ui.label("Install Anni Connect once using that page's instructions. Open INNA, then click Anni Connect → Connect this INNA account.");
                        ui.label("Only your INNA session is handed to this computer. Your Google password stays in your browser.");
                        if ui.button("Cancel browser connection").clicked(){
                            if let Some(rt)=&self.runtime{rt.cancel_browser_auth();}
                            self.auth_in_progress=false;
                            self.browser_auth_url=None;
                            self.state.connection=if self.state.user.is_some(){ConnectionState::SessionExpired}else{ConnectionState::Starting};
                        }
                    }else if let Some(url)=&self.phone_auth_url {
                        theme::page_title(ui,"Connect INNA on your phone","Your phone and computer need to be on the same Wi-Fi network.");
                        ui.label("Open this address on your phone:");
                        ui.label(RichText::new(url).size(24.0).color(theme::ACCENT));
                        ui.label("Choose Open INNA, sign in, and keep the page open until the connection completes.");
                    }else{
                        theme::page_title(ui,"Connecting to INNA…","Finish signing in in the browser window.");
                    }
                    if ui.button("Back to Today").clicked(){self.state.navigate(Page::Home);}
                });
                return;
            }
            if self.state.selected_page != Page::Planner { ui::planner::undo_delete(ui, &mut self.state); }
            let inset = ((ui.available_width() - 1440.0) * 0.5).max(0.0);
            egui::Frame::none().inner_margin(egui::Margin::symmetric(inset, 0.0)).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt(format!("page_{:?}_{:?}", self.state.selected_page, self.state.selected_announcement)).auto_shrink([false,false]).show(ui,|ui| {
                ui.set_width(ui.available_width().min(1440.0));
                match self.state.selected_page {
                    Page::Home=>ui::home::show(ui,&mut self.state),
                    Page::Timetable=>ui::timetable::show(ui,&mut self.state),
                    Page::Announcements=>ui::announcements::show(ui,&mut self.state),
                    Page::Assignments=>ui::assignments::show(ui,&mut self.state),
                    Page::Planner=>ui::planner::show(ui,&mut self.state),
                    Page::Focus=>ui::focus::show(ui,&mut self.state),
                    Page::Connection=>{
                        theme::page_title(ui, "Settings", "Desktop widgets, your INNA connection, and local data.");
                        let desktop_action = self.desktop.settings_ui(ui, self.mock_mode);
                        self.desktop.handle(desktop_action, ctx, &mut self.state);
                        crate::diagnostics::settings_ui(ui, self.mock_mode);
                        let action=ui::connection::show(ui,&mut self.state);self.connection_action(action,ctx);
                    }
                }
            });
            });
        });
        self.finish_update(ctx);
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises eframe's native Close event rather than only checking its command queue.
    #[cfg(windows)]
    #[test]
    #[ignore = "opens temporary mock windows; run with --ignored --test-threads=1"]
    fn native_hide_and_quit_from_a_hidden_planner() {
        native_lifecycle(false);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "opens temporary mock windows; run separately with --ignored --test-threads=1"]
    fn native_close_button_exits_with_widgets_enabled() {
        native_lifecycle(true);
    }

    #[cfg(windows)]
    fn native_lifecycle(close_button: bool) {
        use crate::desktop::DesktopAction;
        use winit::platform::windows::EventLoopBuilderExtWindows;
        const TITLE: &str = "anni-native-lifecycle-test";
        fn root_visible() -> bool {
            let title: Vec<u16> = TITLE.encode_utf16().chain(Some(0)).collect();
            unsafe {
                let hwnd = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(
                    std::ptr::null(),
                    title.as_ptr(),
                );
                !hwnd.is_null()
                    && windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd) != 0
            }
        }
        #[derive(Default)]
        struct Progress {
            stage: u8,
            timed_out: bool,
        }
        struct Harness {
            app: AnniApp,
            progress: Arc<Mutex<Progress>>,
            started: Instant,
            close_button: bool,
        }
        impl eframe::App for Harness {
            fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
                self.app.update(ctx, frame);
                let shared = self.app.shared.clone();
                let progress = self.progress.clone();
                let started = self.started;
                let close_button = self.close_button;
                ctx.show_viewport_deferred(
                    egui::ViewportId::from_hash_of("native_lifecycle_driver"),
                    egui::ViewportBuilder::default()
                        .with_title("Anni lifecycle check")
                        .with_inner_size([240.0, 80.0]),
                    move |ctx, _| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            ui.label("Checking Hide and Quit…");
                        });
                        let elapsed = started.elapsed();
                        let mut progress = progress.lock().unwrap();
                        let mut app = shared.lock().unwrap();
                        if progress.stage == 0 && elapsed.as_millis() >= 500 {
                            assert!(app.desktop.planner_hidden);
                            assert!(!root_visible(), "Desktop startup revealed the planner");
                            let AppData { state, desktop, .. } = &mut *app;
                            desktop.handle(DesktopAction::Disable, ctx, state);
                            app.finish_update(ctx);
                            progress.stage = 1;
                        } else if progress.stage == 1 && elapsed.as_secs_f32() >= 1.0 {
                            assert!(!app.desktop.settings.enabled);
                            assert!(!root_visible(), "Hide widgets revealed the planner");
                            app.desktop.settings.enabled = true;
                            app.finish_update(ctx);
                            progress.stage = 2;
                        } else if progress.stage == 2 && elapsed.as_secs_f32() >= 1.5 {
                            if close_button {
                                app.desktop.planner_hidden = false;
                                ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Visible(true));
                                ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
                                progress.stage = 3;
                            } else {
                            let AppData { state, desktop, .. } = &mut *app;
                            desktop.handle(DesktopAction::Quit, ctx, state);
                            app.finish_update(ctx);
                            progress.stage = 4;
                            }
                        } else if progress.stage == 3 && elapsed.as_secs_f32() >= 2.0 {
                            assert!(root_visible());
                            let title: Vec<u16> = TITLE.encode_utf16().chain(Some(0)).collect();
                            unsafe {
                                use windows_sys::Win32::UI::WindowsAndMessaging::*;
                                let root = FindWindowW(std::ptr::null(), title.as_ptr());
                                PostMessageW(root, WM_CLOSE, 0, 0);
                            }
                            progress.stage = 4;
                        } else if elapsed.as_secs() >= 5 {
                            progress.timed_out = true;
                            ctx.send_viewport_cmd_to(
                                egui::ViewportId::ROOT,
                                egui::ViewportCommand::OuterPosition(egui::pos2(40.0, 40.0)),
                            );
                            ctx.send_viewport_cmd_to(
                                egui::ViewportId::ROOT,
                                egui::ViewportCommand::Visible(true),
                            );
                            ctx.send_viewport_cmd_to(
                                egui::ViewportId::ROOT,
                                egui::ViewportCommand::Close,
                            );
                        }
                        assert!(elapsed.as_secs() < 10, "Native shutdown did not complete");
                        ctx.request_repaint_after(std::time::Duration::from_millis(100));
                    },
                );
            }
        }
        let progress = Arc::new(Mutex::new(Progress::default()));
        let result = progress.clone();
        let options = eframe::NativeOptions {
            persist_window: false,
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            viewport: egui::ViewportBuilder::default()
                .with_title(TITLE)
                .with_inner_size([800.0, 480.0])
                .with_transparent(true),
            ..Default::default()
        };
        eframe::run_native(
            TITLE,
            options,
            Box::new(move |cc| {
                Ok(Box::new(Harness {
                    app: AnniApp::new(cc, true, true),
                    progress,
                    started: Instant::now(),
                    close_button,
                }))
            }),
        )
        .unwrap();
        let progress = result.lock().unwrap();
        assert_eq!(progress.stage, 4);
        assert!(
            !progress.timed_out,
            "Quit needed a visible planner to finish"
        );
    }

    fn test_app() -> (AppData, mpsc::Sender<RuntimeMsg>) {
        let (tx, rx) = mpsc::channel();
        (
            AppData {
                desktop: crate::desktop::Desktop::new(true, false),
                state: AppState::new(),
                mock_mode: false,
                runtime: None,
                rx,
                database: None,
                last_ping: None,
                last_sync: None,
                auth_in_progress: false,
                phone_auth_url: None,
                browser_auth_url: None,
                last_diagnostic: None,
            },
            tx,
        )
    }

    #[test]
    fn desktop_viewport_processes_updates_without_a_planner_frame() {
        desktop_processes_updates(false);
    }

    #[test]
    fn closing_a_newly_focused_planner_quits_instead_of_leaving_a_transparent_window() {
        let (mut app, _) = test_app();
        app.mock_mode = true;
        app.desktop.settings.enabled = true;
        let ctx = egui::Context::default();
        theme::apply_theme(&ctx);
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::WindowFocused(true));
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .events
            .push(egui::ViewportEvent::Close);
        let output = ctx.run(input, |ctx| app.planner_ui(ctx));
        assert!(app.desktop.quit_requested);
        assert!(output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::Close));
        assert!(!output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::CancelClose));
    }

    #[test]
    fn clock_only_desktop_processes_updates_without_a_planner_frame() {
        desktop_processes_updates(true);
    }

    fn desktop_processes_updates(clock_only: bool) {
        let (mut app, tx) = test_app();
        app.mock_mode = true;
        app.desktop.settings.enabled = true;
        app.desktop.settings.show_homework = !clock_only;
        app.desktop.settings.show_schedule = !clock_only;
        let shared = Arc::new(Mutex::new(app));
        let ctx = egui::Context::default();
        theme::apply_theme(&ctx);
        ctx.set_embed_viewports(false);
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            ui::desktop::show(ctx, shared.clone());
        });
        let work_id = egui::ViewportId::from_hash_of(if clock_only {
            "desktop_clock"
        } else {
            "desktop_work"
        });
        let callback = output.viewport_output[&work_id]
            .viewport_ui_cb
            .clone()
            .unwrap();
        tx.send(RuntimeMsg::HomeworkUpdated(mock::mock_homework()))
            .unwrap();
        let mut input = egui::RawInput {
            viewport_id: work_id,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(540.0, 480.0),
            )),
            ..Default::default()
        };
        input
            .viewports
            .insert(work_id, egui::ViewportInfo::default());
        let _ = ctx.run(input, |ctx| callback(ctx));
        assert!(!shared.lock().unwrap().state.homework.is_empty());
        assert!(shared
            .lock()
            .unwrap()
            .state
            .last_sync
            .contains_key("homework"));
    }

    #[test]
    fn completed_study_is_saved_once_and_breaks_do_not_count_as_study() {
        let (mut app, _) = test_app();
        let now = Instant::now();
        app.state.focus.task_title = "Prepare for exam".into();
        app.state.focus.toggle(now);
        app.tick_focus(now + std::time::Duration::from_secs(1501));
        app.tick_focus(now + std::time::Duration::from_secs(1502));
        assert_eq!(app.state.planner.focus_sessions.len(), 1);
        assert_eq!(app.state.planner.focus_sessions[0].minutes, 25);
        assert_eq!(app.state.planner.focus_sessions[0].task, "Prepare for exam");
        assert!(app.state.storage_dirty);
        app.state.storage_dirty = false;
        app.state.focus.preset(5, true);
        app.state.focus.toggle(now);
        app.tick_focus(now + std::time::Duration::from_secs(301));
        assert_eq!(app.state.planner.focus_sessions.len(), 1);
        assert!(!app.state.storage_dirty);
    }

    #[test]
    fn sync_failures_preserve_data_and_successful_empty_responses_replace_it() {
        let (mut app, tx) = test_app();
        let ctx = egui::Context::default();
        app.state.assignments = mock::mock_assignments();
        app.state
            .last_sync
            .insert("assignments".into(), "2026-10-17T12:00:00Z".into());
        app.state.refreshing = true;
        tx.send(RuntimeMsg::SyncError(
            "assignments".into(),
            "network unavailable".into(),
        ))
        .unwrap();
        tx.send(RuntimeMsg::SyncFinished).unwrap();
        app.process_messages(&ctx);
        assert!(!app.state.assignments.is_empty());
        assert!(app.state.sync_errors.contains_key("assignments"));
        assert_eq!(app.state.last_sync["assignments"], "2026-10-17T12:00:00Z");
        assert!(!app.state.refreshing);
        tx.send(RuntimeMsg::AssignmentsUpdated(Vec::new())).unwrap();
        app.process_messages(&ctx);
        assert!(app.state.assignments.is_empty());
        assert!(!app.state.sync_errors.contains_key("assignments"));
        assert!(app.state.storage_dirty);
    }

    #[test]
    fn expired_login_keeps_cached_work_and_personal_tasks_accessible() {
        let (mut app, tx) = test_app();
        app.state.assignments = mock::mock_assignments();
        app.state.planner.add("Study", "Math", "").unwrap();
        tx.send(RuntimeMsg::SessionExpired).unwrap();
        app.process_messages(&egui::Context::default());
        assert_eq!(app.state.connection, ConnectionState::SessionExpired);
        assert!(!app.state.assignments.is_empty());
        assert_eq!(app.state.planner.tasks.len(), 1);
        assert_eq!(app.state.selected_page, Page::Home);
    }

    #[test]
    fn offline_connection_recovers_and_requests_fresh_data() {
        let (mut app, tx) = test_app();
        app.state.connection = ConnectionState::Offline;
        tx.send(RuntimeMsg::PingOk).unwrap();
        app.process_messages(&egui::Context::default());
        assert_eq!(app.state.connection, ConnectionState::Connected);
        assert!(app.state.refresh_requested);
    }
}
