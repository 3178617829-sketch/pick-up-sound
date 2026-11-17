use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::main_window::MAIN_WINDOW_LABEL;

pub const MINI_WINDOW_LABEL: &str = "mini";
// 迷你窗的初始物理尺寸。前端 `ui/mini/components/MiniApp.vue` 的
// `BASE_WIDTH` / `BASE_HEIGHT` 是同一组数字（用来算缩放比），改这里必须同改那边，
// 否则内容会被裁切或留边。
const MINI_WINDOW_WIDTH: f64 = 480.0;
const MINI_WINDOW_HEIGHT: f64 = 224.0;

fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "main window is unavailable".to_owned())
}

fn mini_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(MINI_WINDOW_LABEL)
        .ok_or_else(|| "mini player window is unavailable".to_owned())
}

fn initial_mini_position(main: &WebviewWindow) -> Option<(f64, f64)> {
    let scale_factor = main.scale_factor().ok()?;
    let main_position = main.outer_position().ok()?.to_logical::<f64>(scale_factor);
    let main_size = main.outer_size().ok()?.to_logical::<f64>(scale_factor);
    Some((
        main_position.x + main_size.width - MINI_WINDOW_WIDTH - 24.0,
        main_position.y + main_size.height - MINI_WINDOW_HEIGHT - 48.0,
    ))
}

/// 把迷你窗的坐标夹回主显示器工作区，防止它被算到屏幕外——算出去之后窗口「存在、可见、
/// 但人看不见」，用户只会说「点了没反应」。
fn clamp_to_screen(mini: &WebviewWindow) {
    let Ok(factor) = mini.scale_factor() else {
        return;
    };
    let Ok(pos) = mini.outer_position() else {
        return;
    };
    let Ok(size) = mini.outer_size() else { return };
    let p = pos.to_logical::<f64>(factor);
    let sz = size.to_logical::<f64>(factor);
    let Ok(monitor) = mini.current_monitor() else {
        return;
    };
    let Some(monitor) = monitor else { return };
    let mpos = monitor.position().to_logical::<f64>(factor);
    let msize = monitor.size().to_logical::<f64>(factor);
    let max_x = mpos.x + msize.width - sz.width - 8.0;
    let max_y = mpos.y + msize.height - sz.height - 8.0;
    let x = p.x.clamp(mpos.x + 8.0, max_x.max(mpos.x + 8.0));
    let y = p.y.clamp(mpos.y + 8.0, max_y.max(mpos.y + 8.0));
    if (x - p.x).abs() > 1.0 || (y - p.y).abs() > 1.0 {
        let _ = mini.set_position(tauri::LogicalPosition::new(x, y));
    }
}

/// 打开迷你窗（小组件）。
///
/// ⚠️ 必须是 async + 丢独立线程，不能让同步 command 直接建窗：WebView2 建窗口
/// 要同步等主线程的消息循环，而同步 command 正占着那条线程，两边互等就是死锁。
/// 实测症状：`ui/app.ts` 在启动 1.2 秒后按 localStorage 的 mini-surface-mode
/// 自动调本命令，之后所有 invoke 都不再返回（连不存在的命令也超时），JS 还活着、
/// 进程 Responding、CDP 里多出一个 about:blank。
#[tauri::command]
pub async fn open_mini_player(app: AppHandle) -> Result<(), String> {
    std::thread::spawn(move || {
        if let Err(error) = create_mini_window(&app) {
            eprintln!("[mini] open_mini_player failed: {error}");
        }
    });
    Ok(())
}

/// 建迷你窗。**必须离开 IPC 线程调用**，理由见 open_mini_player。
fn create_mini_window(app: &AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window(MINI_WINDOW_LABEL) {
        // 死句柄：close() 之后窗口对象可能还挂在管理器里，show() 会失败。
        // 这时不能直接返回 Err（用户看到的就是「点了没反应」），要把旧记录清掉重建。
        if existing.show().is_ok() {
            let _ = existing.set_focus();
            clamp_to_screen(&existing);
            return Ok(());
        }
        let _ = existing.destroy();
        std::thread::sleep(std::time::Duration::from_millis(120));
    }

    let builder =
        WebviewWindowBuilder::new(app, MINI_WINDOW_LABEL, WebviewUrl::App("mini.html".into()))
            .title("Pick Up Sound Mini")
            .inner_size(MINI_WINDOW_WIDTH, MINI_WINDOW_HEIGHT)
            .resizable(true)
            .decorations(false)
            .always_on_top(true)
            .visible(false);

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let builder = builder.transparent(true);

    #[cfg(target_os = "windows")]
    let builder = builder.skip_taskbar(true).shadow(true);

    let builder = if let Some((x, y)) = main_window(app)
        .ok()
        .and_then(|main| initial_mini_position(&main))
    {
        builder.position(x, y)
    } else {
        builder
    };

    let mini = builder
        .build()
        .map_err(|error| format!("failed to create mini player window: {error}"))?;
    clamp_to_screen(&mini);

    // 兜底：3 秒内前端没调 mini_player_ready 就强制显示。
    // 创建时是 visible(false) 等前端 ready 再显示（避免白屏闪一下），
    // 但一旦前端加载失败/报错，窗口就永远看不见——用户会觉得「点了没反应」。
    {
        let fallback = mini.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(3000));
            if let Ok(false) = fallback.is_visible() {
                let _ = fallback.show();
            }
        });
    }

    // 拖拽缩放时必须锁住宽高比，否则自由拉伸会把卡片拉变形、内容留白
    let resize_handle = mini.clone();
    mini.on_window_event(move |event| {
        if let tauri::WindowEvent::Resized(size) = event {
            let width = size.width as f64;
            let height = size.height as f64;
            if width < 2.0 || height < 2.0 {
                return;
            }
            let target = (width * MINI_WINDOW_HEIGHT / MINI_WINDOW_WIDTH).round();
            if (height - target).abs() > 3.0 {
                let _ = resize_handle.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                    size.width,
                    target.max(2.0) as u32,
                )));
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub fn mini_player_ready(app: AppHandle) -> Result<(), String> {
    let mini = mini_window(&app)?;
    let main = main_window(&app)?;
    // 顺序很重要：先让迷你窗真的显示出来，再藏主窗口。
    // 反过来的话一旦 show 失败，主窗口已经藏了、迷你窗又没出来，桌面上就空了。
    mini.show().map_err(|error| error.to_string())?;
    clamp_to_screen(&mini);
    mini.set_focus().map_err(|error| error.to_string())?;
    main.hide().map_err(|error| error.to_string())?;
    Ok(())
}

pub fn restore_main_window(app: &AppHandle) -> Result<(), String> {
    let main = main_window(app)?;
    let _ = main.unminimize();
    main.show().map_err(|error| error.to_string())?;
    main.set_focus().map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn exit_mini_player(app: AppHandle) -> Result<(), String> {
    restore_main_window(&app)?;
    if let Some(mini) = app.get_webview_window(MINI_WINDOW_LABEL) {
        mini.close().map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// 按比例缩放迷你窗，保持右下角锚点不动，方便贴在桌面角落。
#[tauri::command]
pub fn set_mini_scale(app: AppHandle, scale: f64) -> Result<(), String> {
    let mini = mini_window(&app)?;
    let scale = scale.clamp(0.7, 1.5);
    let width = MINI_WINDOW_WIDTH * scale;
    let height = MINI_WINDOW_HEIGHT * scale;
    let factor = mini.scale_factor().map_err(|error| error.to_string())?;
    if let (Ok(position), Ok(size)) = (mini.outer_position(), mini.outer_size()) {
        let position = position.to_logical::<f64>(factor);
        let size = size.to_logical::<f64>(factor);
        let x = position.x + size.width - width;
        let y = position.y + size.height - height;
        mini.set_position(tauri::LogicalPosition::new(x, y))
            .map_err(|error| error.to_string())?;
    }
    mini.set_size(tauri::LogicalSize::new(width, height))
        .map_err(|error| error.to_string())?;
    Ok(())
}
