//! 系统托盘：右键「打开主窗口 / 退出」，左键单击直接开主窗口。
//!
//! 主窗口是 hidden 启动的（等前端握手后才 show），任务栏条也只是个附属窗口。
//! 没有托盘的话，窗口一藏进后台就再也没有入口把它叫回来。

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

/// 32x32 PNG 直接嵌进来，不依赖运行时资源目录和 bundle 图标。
const TRAY_ICON: &[u8] = include_bytes!("../icons/32x32.png");

/// 装托盘图标 + 右键菜单（打开主窗口 / 版本 / 退出）。
///
/// 这一段之所以 90 行，是因为每个 `MenuItem::with_id`、`Menu::with_items`、
/// `TrayIconBuilder::build` 都返回 `Result`，而这里的策略统一是：
/// **失败就打一行 `eprintln!` 然后 `return`**。托盘只是附加功能，
/// 建不起来也不能让主程序挂掉——但日志要留，否则「托盘没了」会变成查不出的怪事。
///
/// 菜单里的「版本」项是故意 `enabled=false` 的：用户截个图就能看到磁盘上跑的是哪一版，
/// 配合 `build_info::BUILD_STAMP`（由 `stamp.ps1` 在构建前写进 `.build-stamp`）。
pub fn install(app: &AppHandle) {
    let open = match MenuItem::with_id(app, "open-main", "打开主窗口", true, None::<&str>) {
        Ok(item) => item,
        Err(error) => {
            eprintln!("[tray] open item failed: {error}");
            return;
        }
    };
    let quit = match MenuItem::with_id(app, "quit-app", "退出 Pick Up Sound", true, None::<&str>) {
        Ok(item) => item,
        Err(error) => {
            eprintln!("[tray] quit item failed: {error}");
            return;
        }
    };
    // 版本项不可点，只是一行字：「用户截个图就知道磁盘上跑的是哪版」。
    // 版本号从 Cargo.toml 的 package version 取，和「BFM-1.0」这个对外叫法分开维护：
    // 内部仍是 semver（1.0.0），托盘里去掉末尾的补零显示成 BFM-1.0，更好认。
    let pkg_version = env!("CARGO_PKG_VERSION");
    let short_version = pkg_version.strip_suffix(".0").unwrap_or(pkg_version);
    let version = match MenuItem::with_id(
        app,
        "build-info",
        format!("BFM-{short_version} · {}", crate::build_info::BUILD_STAMP),
        false,
        None::<&str>,
    ) {
        Ok(item) => item,
        Err(error) => {
            eprintln!("[tray] version item failed: {error}");
            return;
        }
    };
    let menu = match Menu::with_items(app, &[&open, &version, &quit]) {
        Ok(menu) => menu,
        Err(error) => {
            eprintln!("[tray] menu failed: {error}");
            return;
        }
    };

    let mut builder = TrayIconBuilder::with_id("pick-up-sound-tray")
        .tooltip("Pick Up Sound")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open-main" => {
                if let Err(error) = crate::main_window::show_main_window(app) {
                    eprintln!("[tray] show main failed: {error}");
                }
            }
            "quit-app" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Err(error) = crate::main_window::show_main_window(tray.app_handle()) {
                    eprintln!("[tray] show main failed: {error}");
                }
            }
        });

    // 优先用 exe 里嵌的窗口图标；拿不到就自己解嵌进来的 PNG。
    let icon = match app.default_window_icon() {
        Some(icon) => Some(icon.clone()),
        None => match image::load_from_memory(TRAY_ICON) {
            Ok(decoded) => {
                let rgba = decoded.to_rgba8();
                let (width, height) = rgba.dimensions();
                Some(tauri::image::Image::new_owned(
                    rgba.into_raw(),
                    width,
                    height,
                ))
            }
            Err(error) => {
                eprintln!("[tray] icon decode failed: {error}");
                None
            }
        },
    };
    if let Some(icon) = icon {
        builder = builder.icon(icon);
    }

    if let Err(error) = builder.build(app) {
        eprintln!("[tray] build failed: {error}");
    }
}
