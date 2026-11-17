fn main() {
    tauri_build::build();

    // 把构建时间戳编进二进制，用于「截图就能认出磁盘上跑的是哪版」。
    // 时间戳由 D:\NDSH\.probe\stamp.ps1 在构建前写入 workspace 根的 .build-stamp；
    // 找不到时退回 "dev"（直接 cargo build 没走 build-bar.bat 的场景）。
    let stamp = std::fs::read_to_string("../.build-stamp")
        .map(|text| text.trim().to_owned())
        .unwrap_or_default();
    let stamp = if stamp.is_empty() {
        "dev".to_owned()
    } else {
        stamp
    };
    println!("cargo:rustc-env=BFM_BUILD_STAMP={stamp}");
    println!("cargo:rerun-if-changed=../.build-stamp");
}
