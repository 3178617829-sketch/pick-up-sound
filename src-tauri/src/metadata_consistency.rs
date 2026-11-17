//! 发布元数据一致性。
//!
//! 版本号在三个地方各写一份：workspace 的 `Cargo.toml`、`src-tauri/Cargo.toml`、
//! `src-tauri/tauri.conf.json`。改一处漏一处不会编译失败，只会让「关于」信息、
//! 安装包版本和 exe 实际版本对不上——这种错只有用户能看见。
//!
//! 这里把它们钉在一起，漏改就红。

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri 必须有父目录")
        .to_path_buf()
}

/// TOML 里第一处形如 `version = "x.y.z"` 的版本号。
fn toml_version(text: &str, label: &str) -> String {
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("version") else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        let value = rest.trim().trim_matches('"').trim_matches('\'');
        if value.starts_with(|c: char| c.is_ascii_digit()) {
            return value.to_owned();
        }
    }
    panic!("{label} 里找不到形如 `version = \"x.y.z\"` 的版本号")
}

/// JSON 里 `"version": "x.y.z"` 的值。
fn json_version(text: &str, label: &str) -> String {
    const MARKER: &str = "\"version\"";
    let start = text
        .find(MARKER)
        .unwrap_or_else(|| panic!("{label} 里找不到 \"version\" 字段"));
    let after = &text[start + MARKER.len()..];
    let colon = after
        .find(':')
        .unwrap_or_else(|| panic!("{label} 的 version 后面缺少冒号"));
    let after_colon = &after[colon + 1..];
    let open = after_colon
        .find('"')
        .unwrap_or_else(|| panic!("{label} 的 version 值缺少开引号"));
    let rest = &after_colon[open + 1..];
    let close = rest
        .find('"')
        .unwrap_or_else(|| panic!("{label} 的 version 值缺少闭引号"));
    rest[..close].to_owned()
}

fn read(path: PathBuf) -> String {
    let display = path.display().to_string();
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("读 {display} 失败：{error}"))
}

#[test]
fn version_is_identical_in_all_three_manifests() {
    let root = repo_root();
    let workspace = toml_version(&read(root.join("Cargo.toml")), "Cargo.toml");
    let tauri_manifest = toml_version(
        &read(root.join("src-tauri/Cargo.toml")),
        "src-tauri/Cargo.toml",
    );
    let tauri_config = json_version(
        &read(root.join("src-tauri/tauri.conf.json")),
        "tauri.conf.json",
    );

    assert_eq!(
        workspace, tauri_manifest,
        "workspace `Cargo.toml` 与 `src-tauri/Cargo.toml` 的 version 不一致"
    );
    assert_eq!(
        workspace, tauri_config,
        "`Cargo.toml` 与 `src-tauri/tauri.conf.json` 的 version 不一致"
    );
    assert!(
        !workspace.is_empty(),
        "版本号不能为空（解析到空值说明格式变了）"
    );
}

#[test]
fn version_parser_rejects_missing_or_malformed_values() {
    // 兜底：解析器自己不能对任何输入都返回空字符串假装通过。
    assert_eq!(toml_version("version = \"2.3.4\"\n", "t"), "2.3.4");
    assert_eq!(json_version("{\"version\": \"2.3.4\"}", "j"), "2.3.4");
    assert!(
        std::panic::catch_unwind(|| toml_version("# 没有版本号\n", "t")).is_err(),
        "没有版本号时应当 panic，而不是返回空字符串"
    );
}
