//! 构建信息：把 `build.rs` 注入的时间戳交给前端显示。
//!
//! 目的是「用户截个图，就能知道磁盘上跑的是哪一版」——时间戳由
//! `D:\NDSH\.probe\stamp.ps1` 在每次构建前刷新，形如 `20261101-0123 a1b2c3d`；
//! 直接 `cargo build` 没走构建脚本时是 `dev`。

/// 编译期注入的构建戳。
pub const BUILD_STAMP: &str = env!("BFM_BUILD_STAMP");

/// 前端在设置页底部显示它。
#[tauri::command]
pub fn build_stamp() -> &'static str {
    BUILD_STAMP
}
