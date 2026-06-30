//! System tray icon and menu

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

/// Create and show the system tray icon, returning the handle
pub fn init_tray(app: &AppHandle) -> anyhow::Result<TrayIcon> {
    let show = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Hide Window", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show, &hide, &quit])?;

    let app_handle = app.clone();
    let tray = TrayIconBuilder::new()
        .tooltip("feiq++")
        .menu(&menu)
        .on_menu_event(move |_tray, event| match event.id.as_ref() {
            "quit" => {
                app_handle.exit(0);
            }
            "show" => {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "hide" => {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(tray)
}

/// Update the tray tooltip to reflect unread count, and (on macOS) the dock badge
pub fn update_tray_badge(tray: &TrayIcon, app_handle: &AppHandle, count: u64) {
    let tooltip = if count > 0 {
        format!("feiq++ ({} unread)", count)
    } else {
        "feiq++".to_string()
    };
    let _ = tray.set_tooltip(Some(&tooltip));

    // macOS dock badge via the main window
    #[cfg(target_os = "macos")]
    {
        let badge_count = if count > 0 { Some(count as i64) } else { None };
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.set_badge_count(badge_count);
        }
    }
}
