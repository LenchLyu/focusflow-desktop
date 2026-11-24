use crate::{
    error::Error, is_presented, launcher_window, show_main, show_mini, show_window, MAIN, MINI,
};
use tauri::{menu::MenuBuilder, tray::TrayIconBuilder, AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

const OPEN_LAUNCHER: &str = "tray-open-launcher";
const OPEN_MAIN: &str = "tray-open-main";
const OPEN_MINI: &str = "tray-open-mini";
const QUIT: &str = "tray-quit";
const TRAY_ID: &str = "focusflow-tray";

#[cfg(target_os = "windows")]
pub(crate) fn is_available(app: &AppHandle) -> bool {
    app.tray_by_id(TRAY_ID).is_some()
}

pub(crate) fn install(app: &AppHandle) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text(OPEN_LAUNCHER, "打开悬浮启动器")
        .text(OPEN_MAIN, "打开主窗口")
        .text(OPEN_MINI, "打开番茄小窗")
        .separator()
        .text(QUIT, "退出 FocusFlow")
        .build()?;

    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("FocusFlow")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            OPEN_LAUNCHER => open(app, open_launcher),
            OPEN_MAIN => open(app, show_main),
            OPEN_MINI => open(app, show_mini),
            QUIT => app.exit(0),
            _ => {}
        });

    // macOS uses its normal menu-bar click behavior. Windows opens the launcher on a left click.
    #[cfg(target_os = "windows")]
    {
        use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
        tray = tray
            .show_menu_on_left_click(false)
            .on_tray_icon_event(|tray, event| {
                if matches!(
                    event,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    }
                ) {
                    open(tray.app_handle(), open_launcher);
                }
            });
    }

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn open_launcher(app: &AppHandle) -> crate::error::Result<()> {
    let anchor = app
        .get_webview_window(MINI)
        .filter(is_presented)
        .or_else(|| app.get_webview_window(MAIN))
        .ok_or(Error::User("无法找到 FocusFlow 主窗口。"))?;
    show_window(&launcher_window(app, &anchor)?)
}

fn open(app: &AppHandle, action: fn(&AppHandle) -> crate::error::Result<()>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = action(&app) {
            app.dialog()
                .message(error.message())
                .title("FocusFlow")
                .kind(MessageDialogKind::Error)
                .show(|_| {});
        }
    });
}
