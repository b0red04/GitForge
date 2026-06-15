use std::env;

/// Why in-app auto-update is unavailable, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateBlockReason {
    CompileTimeExplanation(String),
    RuntimeExplanation(String),
    DebugBuild,
}

impl UpdateBlockReason {
    pub fn message(&self) -> &str {
        match self {
            Self::CompileTimeExplanation(msg) | Self::RuntimeExplanation(msg) => msg,
            Self::DebugBuild => {
                "Auto-updates are disabled in debug builds. Install a release build with the install script."
            }
        }
    }
}

/// Returns `None` when auto-update is allowed for this install.
pub fn update_block_reason() -> Option<UpdateBlockReason> {
    if let Some(explanation) = option_env!("GITFORGE_UPDATE_EXPLANATION") {
        return Some(UpdateBlockReason::CompileTimeExplanation(
            explanation.to_string(),
        ));
    }
    if let Ok(explanation) = env::var("GITFORGE_UPDATE_EXPLANATION") {
        return Some(UpdateBlockReason::RuntimeExplanation(explanation));
    }
    if cfg!(debug_assertions) {
        return Some(UpdateBlockReason::DebugBuild);
    }
    None
}

pub fn auto_update_supported() -> bool {
    update_block_reason().is_none()
}

pub fn default_install_prefix() -> std::path::PathBuf {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        dirs_home().join(".local")
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        dirs::data_local_dir()
            .map(|d| d.join("Programs").join("gitforge"))
            .unwrap_or_else(|| dirs_home())
    }
}

pub fn binary_suffix() -> &'static str {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        "/usr/bin/gitforge"
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        "\\gitforge.exe"
    }
}

pub fn app_folder_name() -> &'static str {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        "gitforge.app"
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        "gitforge"
    }
}

fn dirs_home() -> std::path::PathBuf {
    dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_suffix_is_stable() {
        #[cfg(all(unix, not(target_os = "macos")))]
        assert_eq!(binary_suffix(), "/usr/bin/gitforge");
        #[cfg(windows)]
        assert_eq!(binary_suffix(), "\\gitforge.exe");
    }

    #[test]
    fn app_folder_name_is_stable() {
        #[cfg(all(unix, not(target_os = "macos")))]
        assert_eq!(app_folder_name(), "gitforge.app");
        #[cfg(windows)]
        assert_eq!(app_folder_name(), "gitforge");
    }

    #[test]
    fn debug_build_disables_auto_update() {
        if cfg!(debug_assertions) {
            assert!(!auto_update_supported());
            assert!(matches!(
                update_block_reason(),
                Some(UpdateBlockReason::DebugBuild)
            ));
        }
    }
}
