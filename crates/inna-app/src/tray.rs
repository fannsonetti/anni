//! Anni's icon in the Windows notification area, which stands in for a taskbar button.
//! A click opens the planner; the menu opens it or quits Anni.
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

/// Keep this alive for as long as the icon should stay in the notification area.
pub struct Tray {
    _icon: TrayIcon,
}

/// Create the icon on the thread that runs the window event loop.
pub fn create() -> Option<Tray> {
    let image = crate::ui::brand::icon();
    let icon = match Icon::from_rgba(image.rgba, image.width, image.height) {
        Ok(icon) => icon,
        Err(error) => {
            tracing::warn!("Could not load the tray icon: {error}");
            return None;
        }
    };
    let open = MenuItem::new(crate::locale::t("Open Anni", "Opna Anni"), true, None);
    let quit = MenuItem::new(crate::locale::t("Quit Anni", "Loka Anni"), true, None);
    let menu = Menu::new();
    if let Err(error) = menu.append_items(&[&open, &PredefinedMenuItem::separator(), &quit]) {
        tracing::warn!("Could not build the tray menu: {error}");
        return None;
    }
    let (open_id, quit_id) = (open.id().clone(), quit.id().clone());
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == open_id {
            crate::desktop_platform::reveal_planner();
        } else if event.id == quit_id {
            crate::desktop_platform::close_planner();
        }
    }));
    TrayIconEvent::set_event_handler(Some(|event: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            crate::desktop_platform::reveal_planner();
        }
    }));
    match TrayIconBuilder::new()
        .with_icon(icon)
        .with_tooltip("Anni")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
    {
        Ok(icon) => Some(Tray { _icon: icon }),
        Err(error) => {
            tracing::warn!("Could not add Anni to the notification area: {error}");
            None
        }
    }
}
