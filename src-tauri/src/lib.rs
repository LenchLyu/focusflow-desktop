pub mod backup;
pub mod db;
pub mod error;
pub mod model;
pub mod pomodoro;
pub mod repository;
mod tray;
#[cfg(target_os = "windows")]
mod window_opacity;

use error::{Error, Result};
use model::{Command, MiniView, Pulse, Query, TaskTracking, Workspace};
use repository::{Repository, TimerEvent};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, RunEvent, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_notification::NotificationExt;

type Reply<T> = std::result::Result<T, String>;
const MAIN: &str = "main";
const MINI: &str = "mini";
const LAUNCHER: &str = "launcher";

struct AppState {
    repository: Mutex<Option<Repository>>,
    running: Arc<AtomicBool>,
}

fn data_path(app: &AppHandle) -> Result<PathBuf> {
    let override_path = std::env::var_os("FOCUSFLOW_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::args().find_map(|arg| arg.strip_prefix("--user-data-dir=").map(PathBuf::from))
        });
    let folder = if let Some(path) = override_path {
        if !path.is_absolute() {
            return Err(Error::User("自定义数据目录必须为绝对路径。"));
        }
        path
    } else {
        let base = app
            .path()
            .data_dir()
            .map_err(|_| Error::User("无法定位系统数据目录。"))?;
        base.join(if cfg!(debug_assertions) {
            "FocusFlow Development"
        } else {
            "FocusFlow"
        })
    };
    Ok(folder.join("focusflow.sqlite3"))
}

fn timer_event(app: &AppHandle, event: TimerEvent) {
    if event.sound {
        if let Some(window) = [MINI, MAIN, LAUNCHER]
            .into_iter()
            .filter_map(|label| app.get_webview_window(label))
            .find(is_presented)
            .or_else(|| app.get_webview_window(MAIN))
        {
            if let Err(error) = window.emit("timer-chime", ()) {
                eprintln!("FocusFlow chime: {error}");
            }
        }
    }
    if event.notify {
        if let Err(error) = app
            .notification()
            .builder()
            .title("FocusFlow")
            .body(event.message)
            .show()
        {
            eprintln!("FocusFlow system notification: {error}");
        }
    }
}

fn with_repository<T>(
    app: &AppHandle,
    operation: impl FnOnce(&mut Repository, i64) -> Result<T>,
) -> Result<T> {
    let state = app
        .try_state::<AppState>()
        .ok_or(Error::User("本地数据还未准备好。"))?;
    let mut guard = state
        .repository
        .lock()
        .map_err(|_| Error::User("数据库暂时无法访问，请重启 FocusFlow。"))?;
    let repository = guard.as_mut().ok_or(Error::User("应用正在退出。"))?;
    let now = model::now();
    let event = repository.sync(now)?;
    let result = operation(repository, now);
    drop(guard);
    if let Some(event) = event {
        timer_event(app, event);
    }
    result
}

fn from_window(window: &WebviewWindow, label: &str) -> Reply<()> {
    if window.label() == label {
        Ok(())
    } else {
        Err(Error::User("无法验证操作来源。").ipc())
    }
}

fn input<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Reply<T> {
    serde_json::from_value(value)
        .map_err(|_| Error::User("输入格式或数值超出允许范围，请检查后重试。").ipc())
}

#[tauri::command]
fn workspace_read(
    window: WebviewWindow,
    app: AppHandle,
    query: serde_json::Value,
) -> Reply<Workspace> {
    from_window(&window, MAIN)?;
    let query: Query = input(query)?;
    with_repository(&app, |repo, now| repo.read(query, now)).map_err(Error::ipc)
}

#[tauri::command]
fn workspace_pulse(window: WebviewWindow, app: AppHandle) -> Reply<Pulse> {
    from_window(&window, MAIN)?;
    with_repository(&app, |repo, now| repo.pulse(now)).map_err(Error::ipc)
}

#[tauri::command]
fn workspace_command(
    window: WebviewWindow,
    app: AppHandle,
    command: serde_json::Value,
) -> Reply<()> {
    let command: Command = input(command)?;
    let allowed_in_mini = matches!(
        command,
        Command::Start {
            task_id: None,
            count: 1,
            ..
        } | Command::Advance { .. }
            | Command::Interrupt { .. }
    );
    if window.label() != MINI || !allowed_in_mini {
        from_window(&window, MAIN)?;
    }
    with_repository(&app, |repo, now| repo.command(command, now)).map_err(Error::ipc)
}

#[tauri::command]
fn task_tracking(window: WebviewWindow, app: AppHandle, id: String) -> Reply<TaskTracking> {
    from_window(&window, MAIN)?;
    with_repository(&app, |repo, _| repo.tracking(&id)).map_err(Error::ipc)
}

#[tauri::command]
fn mini_read(window: WebviewWindow, app: AppHandle) -> Reply<MiniView> {
    from_window(&window, MINI)?;
    with_repository(&app, |repo, now| repo.mini(now)).map_err(Error::ipc)
}

#[tauri::command]
async fn set_mini_mode(window: WebviewWindow, app: AppHandle, enabled: bool) -> Reply<()> {
    from_window(&window, if enabled { MAIN } else { MINI })?;
    if enabled {
        show_mini(&app)
    } else {
        show_main(&app)
    }
    .map_err(Error::ipc)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
enum LauncherAction {
    Collapse,
    OpenMain,
    OpenMini,
    Dismiss,
}

#[tauri::command]
async fn launcher_action(
    window: WebviewWindow,
    app: AppHandle,
    action: LauncherAction,
) -> Reply<()> {
    let label = window.label();
    match action {
        LauncherAction::Collapse if matches!(label, MAIN | MINI) => show_launcher(&app, &window),
        LauncherAction::OpenMain if label == LAUNCHER => show_main(&app),
        LauncherAction::OpenMini if label == LAUNCHER => show_mini(&app),
        LauncherAction::Dismiss if label == LAUNCHER => dismiss_launcher(&app),
        _ => Err(Error::User("无法验证操作来源。")),
    }
    .map_err(Error::ipc)
}

#[tauri::command]
fn launcher_theme(window: WebviewWindow, app: AppHandle) -> Reply<model::Theme> {
    from_window(&window, LAUNCHER)?;
    with_repository(&app, |repo, _| Ok(repository::settings(&repo.db)?.theme)).map_err(Error::ipc)
}

#[tauri::command]
async fn backup_export(window: WebviewWindow, app: AppHandle) -> Reply<bool> {
    from_window(&window, MAIN)?;
    tauri::async_runtime::spawn_blocking(move || -> Result<bool> {
        with_repository(&app, |repo, _| backup::ensure_idle(repo))?;
        let chosen = app
            .dialog()
            .file()
            .set_parent(&window)
            .add_filter("JSON 备份", &["json"])
            .set_title("导出 FocusFlow 备份")
            .set_file_name("FocusFlow-backup.json")
            .blocking_save_file();
        let Some(file) = chosen else {
            return Ok(false);
        };
        let path = file
            .into_path()
            .map_err(|_| Error::User("无法读取本地文件路径。"))?;
        let snapshot = with_repository(&app, |repo, _| {
            backup::ensure_idle(repo)?;
            backup::snapshot(&repo.db)
        })?;
        backup::write_file(&path, &snapshot, true)?;
        Ok(true)
    })
    .await
    .map_err(|_| Error::User("备份导出未完成。").ipc())?
    .map_err(Error::ipc)
}

#[tauri::command]
async fn backup_restore(window: WebviewWindow, app: AppHandle) -> Reply<bool> {
    from_window(&window, MAIN)?;
    tauri::async_runtime::spawn_blocking(move || -> Result<bool> {
        with_repository(&app, |repo, _| backup::ensure_idle(repo))?;
        let chosen = app
            .dialog()
            .file()
            .set_parent(&window)
            .add_filter("JSON 备份", &["json"])
            .set_title("选择 FocusFlow 备份")
            .blocking_pick_file();
        let Some(file) = chosen else {
            return Ok(false);
        };
        let path = file
            .into_path()
            .map_err(|_| Error::User("无法读取本地文件路径。"))?;
        let value = backup::read_file(&path)?;
        let approved = app
            .dialog()
            .message("用此备份替换当前应用数据？应用会先保存一份当前数据的 JSON 备份。")
            .title("恢复备份")
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancel)
            .blocking_show();
        if !approved {
            return Ok(false);
        }
        with_repository(&app, |repo, now| backup::restore(repo, value, now))?;
        Ok(true)
    })
    .await
    .map_err(|_| Error::User("备份恢复未完成。").ipc())?
    .map_err(Error::ipc)
}

fn is_presented(window: &WebviewWindow) -> bool {
    window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false)
}

fn show_window(window: &WebviewWindow) -> Result<()> {
    window
        .show()
        .map_err(|_| Error::User("无法显示 FocusFlow 窗口。"))?;
    let _ = window.unminimize();
    let _ = window.set_focus();
    Ok(())
}

fn switch_window(source: &WebviewWindow, target: &WebviewWindow) -> Result<()> {
    show_window(target)?;
    if source.hide().is_err() {
        let _ = target.hide();
        return Err(Error::User("无法切换 FocusFlow 窗口。"));
    }
    Ok(())
}

fn show_main(app: &AppHandle) -> Result<()> {
    let main = app
        .get_webview_window(MAIN)
        .ok_or(Error::User("无法找到 FocusFlow 主窗口。"))?;
    show_window(&main)?;
    if let Some(mini) = app.get_webview_window(MINI) {
        mini.hide().map_err(|_| Error::User("无法隐藏悬浮小窗。"))?;
    }
    Ok(())
}

fn show_mini(app: &AppHandle) -> Result<()> {
    let main = app
        .get_webview_window(MAIN)
        .ok_or(Error::User("无法找到 FocusFlow 主窗口。"))?;
    let mini = match app.get_webview_window(MINI) {
        Some(window) => window,
        None => build_window(app, MINI)?,
    };
    switch_window(&main, &mini)?;
    #[cfg(target_os = "windows")]
    window_opacity::apply(&mini);
    Ok(())
}

fn launcher_window(app: &AppHandle, anchor: &WebviewWindow) -> Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(LAUNCHER) {
        return Ok(window);
    }
    let launcher = build_window(app, LAUNCHER)?;
    if let (Ok(Some(monitor)), Ok(size)) = (anchor.current_monitor(), launcher.outer_size()) {
        let area = monitor.work_area();
        let margin = (16.0 * monitor.scale_factor()).round() as u32;
        let x = area.position.x + area.size.width.saturating_sub(size.width + margin) as i32;
        let y = area.position.y + area.size.height.saturating_sub(size.height + margin) as i32;
        let _ = launcher.set_position(PhysicalPosition::new(x, y));
    }
    Ok(launcher)
}

fn show_launcher(app: &AppHandle, source: &WebviewWindow) -> Result<()> {
    switch_window(source, &launcher_window(app, source)?)
}

fn dismiss_launcher(app: &AppHandle) -> Result<()> {
    if ![MAIN, MINI]
        .into_iter()
        .filter_map(|label| app.get_webview_window(label))
        .any(|window| is_presented(&window))
    {
        show_main(app)?;
    }
    if let Some(launcher) = app.get_webview_window(LAUNCHER) {
        launcher
            .hide()
            .map_err(|_| Error::User("无法关闭悬浮启动器。"))?;
    }
    Ok(())
}

fn build_window(app: &AppHandle, label: &str) -> Result<WebviewWindow> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == label)
        .ok_or(Error::User("找不到 FocusFlow 窗口配置。"))?;
    let dev_origin = app.config().build.dev_url.as_ref().map(|url| url.origin());
    WebviewWindowBuilder::from_config(app, config)
        .map_err(|_| Error::User("无法配置 FocusFlow 窗口。"))?
        .on_navigation(move |url| {
            let local = matches!(
                (url.scheme(), url.host_str()),
                ("tauri", Some("localhost")) | ("http" | "https", Some("tauri.localhost"))
            );
            local
                || (cfg!(debug_assertions)
                    && dev_origin
                        .as_ref()
                        .is_some_and(|origin| *origin == url.origin()))
        })
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
        .build()
        .map_err(|error| {
            eprintln!("FocusFlow window: {error}");
            Error::User("无法创建 FocusFlow 窗口。")
        })
}

fn start(app: &AppHandle) -> Result<()> {
    let mut repository = Repository::open(&data_path(app)?)?;
    let first_event = repository.sync(model::now())?;
    let running = Arc::new(AtomicBool::new(true));
    app.manage(AppState {
        repository: Mutex::new(Some(repository)),
        running: running.clone(),
    });
    build_window(app, MAIN)?;
    if let Err(error) = tray::install(app) {
        eprintln!("FocusFlow tray: {error}");
    }
    show_main(app)?;
    if let Some(event) = first_event {
        timer_event(app, event);
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let mut failed = false;
        while running.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(750));
            if !running.load(Ordering::Acquire) {
                break;
            }
            match with_repository(&app, |_, _| Ok(())) {
                Ok(()) => failed = false,
                Err(error) if !failed => {
                    let _ = error.message();
                    failed = true;
                }
                Err(_) => {}
            }
        }
    });
    Ok(())
}

pub fn run() {
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            let _ = show_main(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            workspace_read,
            workspace_pulse,
            workspace_command,
            task_tracking,
            mini_read,
            set_mini_mode,
            launcher_action,
            launcher_theme,
            backup_export,
            backup_restore
        ])
        .setup(|app| {
            if let Err(error) = start(app.handle()) {
                let handle = app.handle().clone();
                app.dialog()
                    .message(error.message())
                    .title("FocusFlow 无法启动")
                    .kind(MessageDialogKind::Error)
                    .show(move |_| handle.exit(1));
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == LAUNCHER {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = dismiss_launcher(window.app_handle());
                }
                return;
            }
            if window.label() == MINI {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = show_main(window.app_handle());
                }
                return;
            }
            if window.label() != MAIN {
                return;
            }
            match event {
                tauri::WindowEvent::Focused(true) => {
                    let _ = with_repository(window.app_handle(), |_, _| Ok(()));
                }
                #[cfg(target_os = "macos")]
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                #[cfg(target_os = "windows")]
                tauri::WindowEvent::CloseRequested { api, .. }
                    if tray::is_available(window.app_handle()) =>
                {
                    api.prevent_close();
                    if let Err(error) = window.hide() {
                        eprintln!("FocusFlow hide to tray: {error}");
                    }
                }
                #[cfg(target_os = "windows")]
                tauri::WindowEvent::Destroyed => window.app_handle().exit(0),
                _ => {}
            }
        })
        .build(tauri::generate_context!());

    match application {
        Ok(application) => application.run(|app, event| match event {
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => {
                let _ = show_main(app);
            }
            RunEvent::Exit => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.running.store(false, Ordering::Release);
                    if let Ok(mut repository) = state.repository.lock() {
                        repository.take();
                    }
                }
            }
            _ => {}
        }),
        Err(error) => eprintln!("FocusFlow initialization: {error}"),
    }
}
