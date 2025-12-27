fn main() {
    // 跳过 Windows 资源文件嵌入以避免 ICO 格式问题
    #[cfg(not(target_os = "windows"))]
    tauri_build::build()
}
