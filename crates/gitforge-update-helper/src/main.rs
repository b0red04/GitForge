#[cfg(target_os = "windows")]
mod updater;

#[cfg(target_os = "windows")]
fn main() {
    let launch: bool = std::env::args()
        .nth(1)
        .map(|arg| {
            arg == "--launch" && std::env::args().nth(2).map(|v| v == "true").unwrap_or(true)
        })
        .unwrap_or(true);

    let app_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| {
            eprintln!("gitforge-update-helper: cannot determine app directory");
            std::process::exit(1);
        });

    if let Err(e) = updater::perform_update(&app_dir, launch) {
        eprintln!("gitforge-update-helper: update failed: {e:#}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("gitforge-update-helper is only supported on Windows");
    std::process::exit(1);
}
