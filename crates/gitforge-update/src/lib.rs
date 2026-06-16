mod auto_update;
mod detect;
mod github;
mod install;
mod restart;

pub use auto_update::{
    AutoUpdateStatus, AutoUpdater, Check, UpdateCheckType, VersionCheckType, check, init,
    mark_update_ready, notify_if_app_was_updated, set_auto_update_enabled,
};
pub use detect::{auto_update_supported, update_block_reason};
pub use restart::restart_to_apply_update;
