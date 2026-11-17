//! tauri-specta 试点（第 2 轮 2.2）——把 bar 命令的绑定导出成 TypeScript。
//!
//! Builder 定义在 `bar_commands::bar_bindings()`：`collect_commands!` 必须和命令
//! 定义在同一个模块里，否则展开时找不到 `#[tauri::command]` 生成的内部宏。
//! 这里只负责两件事：写文件、验证导出的内容对不对。
//!
//! 运行时注册仍是 `tauri::generate_handler!`，本次不动它——一次把 60 多个命令全迁到
//! `collect_commands!` 属于整体迁移，不是试点。试点要回答的是：依赖能否共存、
//! 标注能否加在带 `AppHandle` 的命令上、8 个平铺参数会不会丢、生成的文件能不能直接用。

/// 把绑定写到 `ui/shared/bindings/bar.ts`。
///
/// 只在 debug 构建里编译：release 包不需要这一步，也不该在启动路径上写文件。
#[cfg(debug_assertions)]
pub fn export_bar_bindings() -> Result<std::path::PathBuf, String> {
    use specta_typescript::Typescript;

    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/shared/bindings/bar.ts");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    crate::bar_commands::bar_bindings()
        .export(Typescript::default(), path.to_string_lossy().into_owned())
        .map_err(|error| error.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    /// 试点验收：能导出，且 5 个命令名与 `push_bar_state` 的 8 个平铺参数一个不丢。
    #[test]
    fn bar_bindings_export_contains_all_five_commands() {
        let dir = std::env::temp_dir().join("bfm-bar-bindings-test");
        std::fs::create_dir_all(&dir).expect("创建临时目录");
        let path = dir.join("bar.ts");
        crate::bar_commands::bar_bindings()
            .export(
                specta_typescript::Typescript::default(),
                path.to_string_lossy().into_owned(),
            )
            .expect("导出 TypeScript 绑定失败");

        let text = std::fs::read_to_string(&path).expect("读回导出的绑定失败");

        for name in [
            "open_taskbar_bar",
            "close_taskbar_bar",
            "taskbar_bar_visible",
            "push_bar_state",
            "bar_control",
        ] {
            assert!(text.contains(name), "导出的绑定里缺少命令 {name}：\n{text}");
        }
        // push_bar_state 的 8 个平铺参数一个都不能丢——这是迁移中最容易出错的地方。
        // 参数名是 camelCase：Tauri v2 的 IPC 约定就是前端传 camelCase（Rust 侧
        // snake_case 自动对应），specta 默认与它一致，所以生成的绑定可以直接
        // 替换手写的 `invoke("push_bar_state", snapshot)`，不用给每个命令加 rename_all。
        for arg in [
            "title", "uploader", "lyric", "playing", "progress", "accent", "barBg", "barText",
        ] {
            assert!(
                text.contains(arg),
                "push_bar_state 的参数 {arg} 没有出现在绑定里：\n{text}"
            );
        }
    }

    /// 仓库里提交的 `ui/shared/bindings/bar.ts` 必须与当前 Rust 签名生成的一致。
    ///
    /// tauri-specta 唯一没解决的短板是「绑定会过期」：改了命令参数却没重新生成，
    /// 前端拿着旧类型调用，编译期看不出、运行期才静默失败。这条测试就是补这个洞。
    #[test]
    fn checked_in_bindings_match_generated_output() {
        let committed =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/shared/bindings/bar.ts");
        let expected = std::fs::read_to_string(&committed)
            .unwrap_or_else(|error| panic!("读不到已提交的绑定 {}：{error}", committed.display()));

        let dir = std::env::temp_dir().join("bfm-bar-bindings-freshness");
        std::fs::create_dir_all(&dir).expect("创建临时目录");
        let path = dir.join("bar.ts");
        crate::bar_commands::bar_bindings()
            .export(
                specta_typescript::Typescript::default(),
                path.to_string_lossy().into_owned(),
            )
            .expect("导出 TypeScript 绑定失败");
        let generated = std::fs::read_to_string(&path).expect("读回导出的绑定失败");

        assert_eq!(
            generated, expected,
            "ui/shared/bindings/bar.ts 与 Rust 签名不一致：重新生成它（跑一次 debug 构建）再提交"
        );
    }
}
