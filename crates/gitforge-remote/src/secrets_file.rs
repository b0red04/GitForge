use std::io::Write;
use std::path::Path;

/// Write JSON to `path` with mode `0o600` on Unix (create + truncate atomically).
pub fn write_restricted_json(path: &Path, json: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(json.as_bytes())?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, json)?;
    }
    Ok(())
}
