use std::ffi::OsStr;

/// Open a URL or file path with the OS default handler.
///
/// URLs open in the default browser, directories open in the file manager,
/// and files open in the default application for their type. Errors are
/// silently ignored (fire-and-forget), matching the previous `xdg-open`
/// call sites.
pub fn open(target: impl AsRef<OsStr>) {
    let target = target.as_ref();
    #[cfg(target_os = "windows")]
    {
        // `cmd /C start "" <target>` — the empty title argument is required
        // when the target is a URL or a path containing spaces.
        let _ = std::process::Command::new("cmd")
            .arg("/C")
            .arg("start")
            .arg("")
            .arg(target)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(target).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(target).spawn();
    }
}
