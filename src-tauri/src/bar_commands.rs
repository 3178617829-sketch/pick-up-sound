//! 任务栏条的前端命令：只做转发，真正的实现在 native_bar（GDI 自绘）。
//!
//! 为什么还叫 open_taskbar_bar / close_taskbar_bar 这一套名字：前端 dist 是
//! 嵌进 exe 的，改名要前后端同时改 + 重发一版。命令名对用户不可见，留着旧名
//! 等于零成本兼容——WebView2 那套实现已经删了，这里只是同名的薄壳。

use tauri::AppHandle;
use tauri_specta::{collect_commands, Builder, Casing, ErrorHandlingMode};

use crate::native_bar;

/// 显示条（WebView2 时代是「建窗口 + 落位」，现在只需把原生窗口亮出来）。
#[tauri::command]
#[specta::specta]
pub fn open_taskbar_bar(_app: AppHandle) -> Result<(), String> {
    native_bar::show();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn close_taskbar_bar(_app: AppHandle) -> Result<(), String> {
    native_bar::hide();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn taskbar_bar_visible(_app: AppHandle) -> bool {
    native_bar::visible()
}

/// 主窗口把歌名 / 播放状态推给条。
#[tauri::command]
#[specta::specta]
pub fn push_bar_state(
    _app: AppHandle,
    title: String,
    uploader: String,
    lyric: String,
    playing: bool,
    progress: f64,
    accent: String,
    bar_bg: String,
    bar_text: String,
) -> Result<(), String> {
    native_bar::update(
        title, uploader, lyric, playing, progress, accent, bar_bg, bar_text,
    );
    Ok(())
}

/// 条上的按钮点一下，转发给主窗口执行（上一首 / 播放暂停 / 下一首）。
#[tauri::command]
#[specta::specta]
pub fn bar_control(_app: AppHandle, action: String) -> Result<(), String> {
    native_bar::forward_control(&action)
}

/// tauri-specta 试点（第 2 轮 2.2）：把本模块这 5 个命令导成一个 Builder。
///
/// 为什么定义在这里而不是单独的 bindings 模块：`collect_commands!` 展开时要引用
/// `#[tauri::command]` 生成的 `__cmd__xxx` / `__tauri_command_name_xxx` 内部宏，
/// 而这些宏的作用域就是命令所在的模块，换个模块展开会报 `cannot find macro`。
pub fn bar_bindings() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        // 默认会把命令名转成 camelCase（`commands.openTaskbarBar`）；前端现在 invoke
        // 的是 snake_case，这里锁死蛇形，将来整体迁移时零改名。
        .function_casing(Casing::SnakeCase)
        .error_handling(ErrorHandlingMode::ResultUnknown)
        .commands(collect_commands![
            open_taskbar_bar,
            close_taskbar_bar,
            taskbar_bar_visible,
            push_bar_state,
            bar_control,
        ])
}
