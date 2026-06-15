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
  dirs_home().join(".local")
}

pub fn binary_suffix() -> &'static str {
  "/usr/bin/gitforge"
}

pub fn app_folder_name() -> &'static str {
  "gitforge.app"
}

fn dirs_home() -> std::path::PathBuf {
  env::var_os("HOME")
    .map(std::path::PathBuf::from)
    .unwrap_or_else(|| std::path::PathBuf::from("."))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn binary_suffix_is_stable() {
    assert_eq!(binary_suffix(), "/usr/bin/gitforge");
  }

  #[test]
  fn app_folder_name_is_stable() {
    assert_eq!(app_folder_name(), "gitforge.app");
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
