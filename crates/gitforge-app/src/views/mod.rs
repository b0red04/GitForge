pub mod app;
pub mod command_palette;
pub mod commands;
pub mod commit_editor;
pub mod dialogs;
pub mod diff_panel;
pub mod diff_view;
pub mod diff_viewer;
pub mod graph_panel;
pub mod layout;
pub mod ops;
pub mod repo_session;
pub mod repo_tabs;
pub mod settings;
pub mod settings_window;
pub mod sidebar;
pub mod status_panel;
pub mod titlebar;
pub mod toasts;
pub mod toolbar;
pub mod window_chrome;

pub use app::GitForgeApp;
pub use app::ShowCommandPalette;
pub use app::ToggleTheme;
pub use app::{CloseDialog, OpenRepository, SelectNextCommit, SelectPrevCommit};
pub use app::{
    CloseTab, InitRepo, NewTab, OpenInEditor, OpenInFileManager, OpenInTerminal,
    OpenRepoManagement, Preferences, QuitApp,
};
pub use app::{CreateBranch, StashPop, StashPush};
pub use app::{FetchAll, PullCurrent, PushCurrent};
pub use settings_window::SettingsSection;
