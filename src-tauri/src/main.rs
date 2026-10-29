// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    configure_linux_rendering();

    app_lib::run()
}

#[cfg(target_os = "linux")]
fn configure_linux_rendering() {
    // Set WebKit options before Tauri initializes GTK or starts any threads.
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none()
    {
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }

    // NVIDIA's GBM/EGL path can abort WebKitGTK before a window appears, on
    // both Wayland and X11: https://bugs.webkit.org/show_bug.cgi?id=280210
    // Detect the loaded driver rather than requiring nvidia-smi in AppImages.
    let nvidia_loaded = std::path::Path::new("/proc/driver/nvidia/version").exists()
        || std::path::Path::new("/sys/module/nvidia").exists();
    if nvidia_loaded && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        eprintln!("TupleDB: NVIDIA detected; applying WEBKIT_DISABLE_DMABUF_RENDERER=1");
    }
}
