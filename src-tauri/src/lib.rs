use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;
use tauri::State;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// 设置文件路径（不依赖 AppHandle）
fn settings_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var_os("APPDATA").unwrap_or_default();
        PathBuf::from(appdata).join("ScriptController").join("settings.json")
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
        home.join(".config").join("ScriptController").join("settings.json")
    }
}

fn load_close_to_tray() -> bool {
    let path = settings_path();
    let Ok(s) = fs::read_to_string(&path) else { return true };
    let Ok(j) = serde_json::from_str::<serde_json::Value>(&s) else { return true };
    j.get("closeToTray").and_then(|v| v.as_bool()).unwrap_or(true)
}

fn save_close_to_tray(value: bool) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let j = serde_json::json!({ "closeToTray": value });
    let _ = fs::write(path, j.to_string());
}

const API_PORT_MIN: u16 = 8765;
const API_PORT_MAX: u16 = 8775;

/// 托盘图标（与窗口图标一致，32x32 适合任务栏）
const TRAY_ICON: tauri::image::Image<'static> = tauri::include_image!("icons/32x32.png");

/// 单实例：锁文件存 PID，若文件存在且该进程存活则退出
fn try_single_instance() -> bool {
    let lock_path = std::env::temp_dir().join("script_controller_app.lock");
    if let Ok(old) = fs::read_to_string(&lock_path) {
        if let Ok(pid) = old.trim().parse::<u32>() {
            #[cfg(target_os = "windows")]
            {
                let out = Command::new("cmd")
                    .args(["/C", "tasklist", "/FI", &format!("PID eq {}", pid), "/NH"])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output();
                if let Ok(o) = out {
                    let out_str = String::from_utf8_lossy(&o.stdout);
                    if out_str.contains(&pid.to_string()) {
                        return false;
                    }
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = std::process::Command::new("kill").args(["-0", &pid.to_string()]).output();
            }
        }
    }
    let pid = std::process::id();
    let _ = fs::File::create(&lock_path).and_then(|mut f| f.write_all(pid.to_string().as_bytes()));
    true
}

/// 应用状态：Python 是否可用、API 端口、后端子进程、关闭时是否最小化到托盘
pub struct AppState {
    pub python_ok: bool,
    pub api_port: Option<u16>,
    pub backend_process: Mutex<Option<Child>>,
    pub startup_error: Option<String>,
    pub close_to_tray: Mutex<bool>,
}

/// 解析 Python 可执行文件完整路径（直接运行 exe 时与终端环境一致）
fn get_python_exe() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let out = Command::new("cmd")
            .args(["/C", "where python"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        if !out.status.success() || out.stdout.is_empty() {
            return None;
        }
        let first = std::str::from_utf8(&out.stdout)
            .ok()?
            .lines()
            .next()?
            .trim();
        if first.is_empty() {
            return None;
        }
        Some(first.to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let out = Command::new("sh")
            .args(["-c", "which python3 2>/dev/null || which python 2>/dev/null"])
            .output()
            .ok()?;
        if !out.status.success() || out.stdout.is_empty() {
            return None;
        }
        let first = std::str::from_utf8(&out.stdout).ok()?.lines().next()?.trim();
        if first.is_empty() {
            return None;
        }
        Some(first.to_string())
    }
}

/// 检测本机是否有 Python（Windows: where python）
fn check_python() -> bool {
    get_python_exe().is_some()
}

/// 项目根目录（script_controller 所在目录），后端 run.py 的 cwd
/// 优先：环境变量 SCRIPT_CONTROLLER_PROJECT_ROOT > exe 同目录 script_controller_root.txt > 默认
fn project_root() -> String {
    if let Ok(root) = std::env::var("SCRIPT_CONTROLLER_PROJECT_ROOT") {
        let root = root.trim();
        if !root.is_empty() {
            return root.to_string();
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let cfg_path = dir.join("script_controller_root.txt");
            if let Ok(s) = fs::read_to_string(&cfg_path) {
                let s = s.trim();
                if !s.is_empty() {
                    return s.to_string();
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    return "D:\\project".to_string();
    #[cfg(not(target_os = "windows"))]
    return "/tmp/project".to_string();
}

/// CREATE_NO_WINDOW：启动子进程时不弹出 CMD 窗口（仅 Windows）
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 启动后端：用解析到的 Python 完整路径执行 -m script_controller.run，cwd 为项目根，并强制数据根为 script_controller 目录
fn start_backend() -> Result<Child, String> {
    let root = project_root();
    let script_controller_root = std::path::Path::new(&root).join("script_controller");
    let python_exe = get_python_exe().unwrap_or_else(|| "python".to_string());
    let mut cmd = Command::new(&python_exe);
    cmd.args(["-m", "script_controller.run"])
        .current_dir(&root)
        .env("SCRIPT_CONTROLLER_ROOT", script_controller_root.to_string_lossy().to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let child = cmd.spawn().map_err(|e| format!("启动后端失败: {}", e))?;
    Ok(child)
}

/// 轮询 health 接口，返回实际端口（仅信任 API_PORT_MIN..=API_PORT_MAX，避免误用 5173 等）
fn wait_for_port(max_wait_ms: u64) -> Option<u16> {
    let ports: Vec<u16> = (API_PORT_MIN..=API_PORT_MAX).collect();
    let step = 300u64;
    let mut elapsed = 0u64;
    while elapsed < max_wait_ms {
        for &port in &ports {
            if let Ok(resp) = reqwest::blocking::get(format!("http://127.0.0.1:{}/api/health", port)) {
                if resp.status().is_success() {
                    if let Ok(j) = resp.json::<serde_json::Value>() {
                        if let Some(p) = j.get("port").and_then(|v| v.as_u64()) {
                            let p = p as u16;
                            if (API_PORT_MIN..=API_PORT_MAX).contains(&p) {
                                return Some(p);
                            }
                        }
                        return Some(port);
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(step));
        elapsed += step;
    }
    None
}

/// 结束后端进程（Windows 下杀进程树）
fn kill_backend(child: &mut Child) {
    #[cfg(target_os = "windows")]
    {
        let pid = child.id();
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

/// 退出前清理：结束后端、删除单实例锁
fn cleanup_on_exit(state: &AppState) {
    if let Ok(mut guard) = state.backend_process.lock() {
        if let Some(mut child) = guard.take() {
            kill_backend(&mut child);
        }
    }
    let _ = fs::remove_file(std::env::temp_dir().join("script_controller_app.lock"));
}

/// 前端调用的命令：由 Rust 代理请求后端 API，避免打包后 CORS（tauri://localhost 被后端拒绝）
#[tauri::command]
fn fetch_api(
    state: State<AppState>,
    path: String,
    method: String,
    body: Option<String>,
) -> Result<String, String> {
    let port = state
        .api_port
        .ok_or_else(|| "后端未就绪".to_string())?;
    if !(API_PORT_MIN..=API_PORT_MAX).contains(&port) {
        return Err(format!(
            "API 端口 {} 不在预期范围 {}-{}，请勿使用开发服务器端口（如 5173）。",
            port, API_PORT_MIN, API_PORT_MAX
        ));
    }
    let path = path.trim_start_matches('/');
    let url = format!("http://127.0.0.1:{}/{}", port, path);
    let method = method.to_uppercase();
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let req = match method.as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        _ => return Err(format!("不支持的 method: {}", method)),
    };
    let req = req.header("Accept", "application/json");
    let req = if let Some(b) = body {
        req.header("Content-Type", "application/json").body(b)
    } else {
        req
    };
    let resp = req.send().map_err(|e| e.to_string())?;
    let status = resp.status();
    let text = resp.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("HTTP {}: {}", status.as_u16(), text));
    }
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        if trimmed.starts_with('<') {
            return Err(format!(
                "后端返回了 HTML 而非 JSON，请确认 {} 是 API 地址。摘要：{}",
                url,
                trimmed.chars().take(80).collect::<String>()
            ));
        }
        if serde_json::from_str::<serde_json::Value>(trimmed).is_err() {
            return Err(format!(
                "后端返回了非 JSON 数据，请确认脚本控制器已正确启动。响应摘要：{}",
                trimmed.chars().take(120).collect::<String>()
            ));
        }
    }
    Ok(text)
}

/// 前端调用的命令：打开开发者工具（调试样式等）
#[tauri::command]
fn open_devtools(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        w.open_devtools();
    }
}

/// 前端调用的命令：返回启动状态（是否检测到 Python、API 端口、错误信息）
#[tauri::command]
fn get_startup_status(state: State<AppState>) -> Result<serde_json::Value, String> {
    let out = serde_json::json!({
        "python_ok": state.python_ok,
        "api_port": state.api_port,
        "error": state.startup_error
    });
    Ok(out)
}

/// 获取「关闭窗口时最小化到托盘」选项
#[tauri::command]
fn get_close_behavior(state: State<AppState>) -> Result<bool, String> {
    Ok(*state.close_to_tray.lock().map_err(|e| e.to_string())?)
}

/// 设置「关闭窗口时最小化到托盘」选项并持久化
#[tauri::command]
fn set_close_behavior(state: State<AppState>, value: bool) -> Result<(), String> {
    {
        let mut g = state.close_to_tray.lock().map_err(|e| e.to_string())?;
        *g = value;
    }
    save_close_to_tray(value);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if !try_single_instance() {
        eprintln!("已有实例在运行");
        std::process::exit(0);
    }
    let python_ok = check_python();
    let (api_port, backend_process, startup_error) = if !python_ok {
        (
            None,
            None,
            Some("未检测到 Python，请安装 Python 3.9+ 并加入 PATH。".to_string()),
        )
    } else {
        match start_backend() {
            Ok(mut child) => {
                let port = wait_for_port(15000);
                if let Some(p) = port {
                    let _ = child.stderr.take();
                    (Some(p), Some(child), None)
                } else {
                    let mut err_msg = "后端启动超时或未就绪。".to_string();
                    if let Some(mut stderr) = child.stderr.take() {
                        let mut buf = String::new();
                        let _ = std::io::Read::read_to_string(&mut stderr, &mut buf);
                        let buf = buf.trim();
                        if !buf.is_empty() {
                            let tail = if buf.len() > 600 {
                                format!("...{}", &buf[buf.len() - 600..])
                            } else {
                                buf.to_string()
                            };
                            err_msg.push_str("\n\n后端输出：");
                            err_msg.push_str(&tail.replace('\r', ""));
                        }
                    }
                    let _ = child.kill();
                    (None, None, Some(err_msg))
                }
            }
            Err(e) => (None, None, Some(e)),
        }
    };

    let state = AppState {
        python_ok,
        api_port,
        backend_process: Mutex::new(backend_process),
        startup_error,
        close_to_tray: Mutex::new(load_close_to_tray()),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_startup_status,
            open_devtools,
            fetch_api,
            get_close_behavior,
            set_close_behavior,
        ])
        .setup(move |app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            // 系统托盘：右键菜单「显示主界面」「退出」
            let show_item = MenuItem::with_id(app.handle(), "show", "显示主界面", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app.handle(), "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(
                app.handle(),
                &[&show_item, &quit_item],
            )?;
            let _tray = TrayIconBuilder::new()
                .icon(TRAY_ICON.clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("脚本控制")
                .on_menu_event(move |app_handle, event| {
                    let id = event.id().as_ref();
                    if id == "show" {
                        if let Some(w) = app_handle.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    } else if id == "quit" {
                        if let Some(state) = app_handle.try_state::<AppState>() {
                            cleanup_on_exit(&state);
                        }
                        let _ = app_handle.exit(0);
                    }
                })
                .build(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    let do_tray = match window.app_handle().try_state::<AppState>() {
                        Some(state) => *state.close_to_tray.lock().unwrap_or_else(|e| e.into_inner()),
                        None => true,
                    };
                    if do_tray {
                        window.hide().ok();
                        api.prevent_close();
                    }
                }
                tauri::WindowEvent::Destroyed => {
                    let state = window.app_handle().state::<AppState>();
                    cleanup_on_exit(&state);
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
