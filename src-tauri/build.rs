fn main() {
    // 图标变化时强制重新嵌入资源（默认 Cargo 不监视 icons/，
    // 重新生成的 .ico 不会进 exe，运行中的窗口/托盘仍是旧图标）
    println!("cargo:rerun-if-changed=icons/");
    tauri_build::build()
}
