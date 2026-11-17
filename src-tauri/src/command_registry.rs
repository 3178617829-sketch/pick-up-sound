//! 命令注册表一致性检查。
//!
//! `main.rs` 的 `generate_handler![...]` 是一长串手写列表。新增一个
//! `#[tauri::command]` 却忘了注册，编译期完全看不出来——前端 `invoke`
//! 时才会收到「command not found」，而且大概率被 `.catch(() => {})` 吞掉。
//! 这个测试把「声明了的命令」和「注册了的命令」各自收集起来比对，
//! 两边任何一边多出来都会红。
//!
//! 命名规则同时钉在这里：命令名用 `动词_名词` 的小写下划线形式
//! （`open_taskbar_bar` / `push_bar_state` / `set_taskbar_playback_state`）。

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    /// `src/` 下全部 `.rs` 文件。
    fn rust_sources() -> Vec<PathBuf> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("读不到源码目录") {
                let path = entry.expect("目录项读取失败").path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    out.push(path);
                }
            }
        }
        let mut out = Vec::new();
        walk(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut out,
        );
        out
    }

    /// 从 `fn foo<T>(...)` / `pub fn foo(...)` 这类行里取函数名。
    fn fn_name(line: &str) -> Option<String> {
        let rest = line.split_once("fn ")?.1;
        let end = rest.find(['(', '<', ' ', ':'])?;
        let name = &rest[..end];
        (!name.is_empty()).then(|| name.to_owned())
    }

    /// 所有被 `#[tauri::command]` 标注的函数名。
    fn declared_commands() -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        for path in rust_sources() {
            let text = std::fs::read_to_string(&path).expect("源码读取失败");
            let mut lines = text.lines();
            while let Some(line) = lines.next() {
                if !line.trim_start().starts_with("#[tauri::command") {
                    continue;
                }
                // 属性之后可能还夹着别的属性（如 #[cfg(...)]），继续往下找 fn。
                for candidate in lines.by_ref() {
                    let trimmed = candidate.trim_start();
                    if let Some(name) = fn_name(trimmed) {
                        found.insert(name);
                        break;
                    }
                    if !trimmed.starts_with('#') && !trimmed.is_empty() {
                        break;
                    }
                }
            }
        }
        found
    }

    /// `generate_handler![...]` 的方括号之间那段（按括号配对扫描，
    /// 列表里会夹 `#[cfg(debug_assertions)]`，它的 `]` 不能被当成闭合）。
    fn handler_block(text: &str) -> &str {
        let open = text
            .find("generate_handler![")
            .expect("main.rs 里找不到 generate_handler![")
            + "generate_handler![".len();
        let mut depth = 1usize;
        for (offset, byte) in text.as_bytes()[open..].iter().enumerate() {
            match byte {
                b'[' => depth += 1,
                b']' => {
                    depth -= 1;
                    if depth == 0 {
                        return &text[open..open + offset];
                    }
                }
                _ => {}
            }
        }
        panic!("generate_handler![ 没有闭合的 ]")
    }

    /// `generate_handler![...]` 里注册的命令名（取路径最后一段）。
    fn registered_commands() -> BTreeSet<String> {
        let main_rs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/main.rs");
        let text = std::fs::read_to_string(main_rs).expect("main.rs 读取失败");
        handler_block(&text)
            .lines()
            .map(|line| line.split("//").next().unwrap_or(""))
            .flat_map(|line| line.split(','))
            .map(str::trim)
            // 条件编译属性不是命令。
            .filter(|item| !item.is_empty() && !item.starts_with("#["))
            .map(|item| item.rsplit("::").next().unwrap_or(item).to_owned())
            .collect()
    }

    #[test]
    fn every_declared_command_is_registered() {
        let missing: Vec<String> = declared_commands()
            .difference(&registered_commands())
            .cloned()
            .collect();
        assert!(
            missing.is_empty(),
            "这些函数标了 #[tauri::command] 但没注册进 generate_handler!，前端调用会静默失败：{missing:?}"
        );
    }

    #[test]
    fn every_registered_command_is_declared() {
        let extra: Vec<String> = registered_commands()
            .difference(&declared_commands())
            .cloned()
            .collect();
        assert!(
            extra.is_empty(),
            "generate_handler! 里注册了不存在的命令：{extra:?}"
        );
    }

    #[test]
    fn registry_scan_actually_finds_commands() {
        // 扫描逻辑本身失效（比如属性写法变了）时，前两个断言会因为两边都空而假装通过。
        let declared = declared_commands();
        assert!(
            declared.len() > 20,
            "只扫到 {} 个命令，扫描逻辑可能已失效",
            declared.len()
        );
    }
}
