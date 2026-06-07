use gpui::Action;

use crate::views::app::{
    OpenRepository, SelectPrevCommit, SelectNextCommit, BackToDiff,
    ShowStatusPanel, ShowHistory, RefreshRepository, SoftReset, CreateBranch, StashPush, StashPop,
    ToggleTheme, ShowCommandPalette, NewTab, CloseTab,
    ReopenClosedTab, InitRepo, OpenRepoManagement, OpenInEditor, OpenInTerminal,
    OpenInFileManager, OpenInBrowser, Preferences, QuitApp, CloneRepo, CloneFromGithub,
    CloneFromGitlab, AddRemote, CreateWorktree, OpenSshKey, ManageAccounts, OpenAiSettings,
    ViewFileAtCommit,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitlebarMenu {
    File,
    Edit,
    Selection,
    View,
}

impl TitlebarMenu {
    pub const ALL: [TitlebarMenu; 4] = [
        TitlebarMenu::File,
        TitlebarMenu::Edit,
        TitlebarMenu::Selection,
        TitlebarMenu::View,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TitlebarMenu::File => "File",
            TitlebarMenu::Edit => "Edit",
            TitlebarMenu::Selection => "Selection",
            TitlebarMenu::View => "View",
        }
    }

    pub fn element_id(self) -> &'static str {
        match self {
            TitlebarMenu::File => "titlebar-menu-file",
            TitlebarMenu::Edit => "titlebar-menu-edit",
            TitlebarMenu::Selection => "titlebar-menu-selection",
            TitlebarMenu::View => "titlebar-menu-view",
        }
    }

    pub fn dropdown_left(self) -> f32 {
        match self {
            TitlebarMenu::File => 44.0,
            TitlebarMenu::Edit => 84.0,
            TitlebarMenu::Selection => 124.0,
            TitlebarMenu::View => 210.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandAction {
    NewTab,
    CloseTab,
    ReopenClosedTab,
    Clone,
    InitRepo,
    OpenRepository,
    RepoManagement,
    CloneGithub,
    CloneGitlab,
    AddRemote,
    OpenEditor,
    OpenTerminal,
    OpenFileManager,
    OpenBrowser,
    Worktree,
    Preferences,
    Accounts,
    Quit,
    Refresh,
    CreateBranch,
    StashPush,
    StashPop,
    SoftReset,
    AiSettings,
    SshKey,
    SelectPrev,
    SelectNext,
    ViewFile,
    BackToDiff,
    ShowHistory,
    ShowStatus,
    CommandPalette,
    ToggleTheme,
}

impl CommandAction {
    pub fn boxed_action(self) -> Box<dyn Action> {
        match self {
            Self::NewTab => Box::new(NewTab),
            Self::CloseTab => Box::new(CloseTab),
            Self::ReopenClosedTab => Box::new(ReopenClosedTab),
            Self::Clone => Box::new(CloneRepo),
            Self::InitRepo => Box::new(InitRepo),
            Self::OpenRepository => Box::new(OpenRepository),
            Self::RepoManagement => Box::new(OpenRepoManagement),
            Self::CloneGithub => Box::new(CloneFromGithub),
            Self::CloneGitlab => Box::new(CloneFromGitlab),
            Self::AddRemote => Box::new(AddRemote),
            Self::OpenEditor => Box::new(OpenInEditor),
            Self::OpenTerminal => Box::new(OpenInTerminal),
            Self::OpenFileManager => Box::new(OpenInFileManager),
            Self::OpenBrowser => Box::new(OpenInBrowser),
            Self::Worktree => Box::new(CreateWorktree),
            Self::Preferences => Box::new(Preferences),
            Self::Accounts => Box::new(ManageAccounts),
            Self::Quit => Box::new(QuitApp),
            Self::Refresh => Box::new(RefreshRepository),
            Self::CreateBranch => Box::new(CreateBranch),
            Self::StashPush => Box::new(StashPush),
            Self::StashPop => Box::new(StashPop),
            Self::SoftReset => Box::new(SoftReset),
            Self::AiSettings => Box::new(OpenAiSettings),
            Self::SshKey => Box::new(OpenSshKey),
            Self::SelectPrev => Box::new(SelectPrevCommit),
            Self::SelectNext => Box::new(SelectNextCommit),
            Self::ViewFile => Box::new(ViewFileAtCommit),
            Self::BackToDiff => Box::new(BackToDiff),
            Self::ShowHistory => Box::new(ShowHistory),
            Self::ShowStatus => Box::new(ShowStatusPanel),
            Self::CommandPalette => Box::new(ShowCommandPalette),
            Self::ToggleTheme => Box::new(ToggleTheme),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CommandEntry {
    pub label: &'static str,
    pub action: CommandAction,
    pub keybinding: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
pub enum MenuEntry {
    Item(CommandEntry),
    Separator,
}

const FILE_MENU: &[MenuEntry] = &[
    MenuEntry::Item(CommandEntry {
        label: "New Tab",
        action: CommandAction::NewTab,
        keybinding: Some("Ctrl+T"),
    }),
    MenuEntry::Item(CommandEntry {
        label: "Close Tab",
        action: CommandAction::CloseTab,
        keybinding: Some("Ctrl+W"),
    }),
    MenuEntry::Item(CommandEntry {
        label: "Reopen Closed Tab",
        action: CommandAction::ReopenClosedTab,
        keybinding: None,
    }),
    MenuEntry::Separator,
    MenuEntry::Item(CommandEntry {
        label: "Clone Repo...",
        action: CommandAction::Clone,
        keybinding: None,
    }),
    MenuEntry::Item(CommandEntry {
        label: "Init Repo...",
        action: CommandAction::InitRepo,
        keybinding: Some("Ctrl+I"),
    }),
    MenuEntry::Item(CommandEntry {
        label: "Open Repo...",
        action: CommandAction::OpenRepository,
        keybinding: Some("Ctrl+O"),
    }),
    MenuEntry::Item(CommandEntry {
        label: "Open Repo Management",
        action: CommandAction::RepoManagement,
        keybinding: Some("Alt+Ctrl+O"),
    }),
    MenuEntry::Item(CommandEntry {
        label: "Clone from GitHub",
        action: CommandAction::CloneGithub,
        keybinding: None,
    }),
    MenuEntry::Item(CommandEntry {
        label: "Clone from GitLab",
        action: CommandAction::CloneGitlab,
        keybinding: None,
    }),
    MenuEntry::Item(CommandEntry {
        label: "Add Remote",
        action: CommandAction::AddRemote,
        keybinding: None,
    }),
    MenuEntry::Separator,
    MenuEntry::Item(CommandEntry {
        label: "Open Repo in External Editor",
        action: CommandAction::OpenEditor,
        keybinding: Some("Ctrl+Shift+E"),
    }),
    MenuEntry::Item(CommandEntry {
        label: "Open External Terminal",
        action: CommandAction::OpenTerminal,
        keybinding: Some("Alt+T"),
    }),
    MenuEntry::Item(CommandEntry {
        label: "Open in File Manager",
        action: CommandAction::OpenFileManager,
        keybinding: Some("Alt+O"),
    }),
    MenuEntry::Item(CommandEntry {
        label: "Open in Browser",
        action: CommandAction::OpenBrowser,
        keybinding: None,
    }),
    MenuEntry::Item(CommandEntry {
        label: "Create Worktree",
        action: CommandAction::Worktree,
        keybinding: None,
    }),
    MenuEntry::Separator,
    MenuEntry::Item(CommandEntry {
        label: "Preferences...",
        action: CommandAction::Preferences,
        keybinding: Some("Ctrl+,"),
    }),
    MenuEntry::Separator,
    MenuEntry::Item(CommandEntry {
        label: "Sign into a Different Account",
        action: CommandAction::Accounts,
        keybinding: None,
    }),
    MenuEntry::Item(CommandEntry {
        label: "Quit GitForge",
        action: CommandAction::Quit,
        keybinding: Some("Ctrl+Q"),
    }),
];

const EDIT_MENU: &[CommandEntry] = &[
    CommandEntry {
        label: "Refresh Repository",
        action: CommandAction::Refresh,
        keybinding: None,
    },
    CommandEntry {
        label: "Create Branch",
        action: CommandAction::CreateBranch,
        keybinding: Some("Ctrl+N"),
    },
    CommandEntry {
        label: "Stash Changes",
        action: CommandAction::StashPush,
        keybinding: Some("Ctrl+Shift+S"),
    },
    CommandEntry {
        label: "Pop Stash",
        action: CommandAction::StashPop,
        keybinding: Some("Ctrl+Shift+O"),
    },
    CommandEntry {
        label: "Undo Last Commit",
        action: CommandAction::SoftReset,
        keybinding: None,
    },
    CommandEntry {
        label: "AI Settings",
        action: CommandAction::AiSettings,
        keybinding: None,
    },
    CommandEntry {
        label: "Manage Accounts",
        action: CommandAction::Accounts,
        keybinding: None,
    },
    CommandEntry {
        label: "Generate SSH Key",
        action: CommandAction::SshKey,
        keybinding: None,
    },
];

const SELECTION_MENU: &[CommandEntry] = &[
    CommandEntry {
        label: "Select Previous Commit",
        action: CommandAction::SelectPrev,
        keybinding: Some("Up"),
    },
    CommandEntry {
        label: "Select Next Commit",
        action: CommandAction::SelectNext,
        keybinding: Some("Down"),
    },
    CommandEntry {
        label: "View File at Commit",
        action: CommandAction::ViewFile,
        keybinding: Some("Enter"),
    },
    CommandEntry {
        label: "Back to Diff",
        action: CommandAction::BackToDiff,
        keybinding: Some("Escape"),
    },
];

const VIEW_MENU: &[CommandEntry] = &[
    CommandEntry {
        label: "Show History",
        action: CommandAction::ShowHistory,
        keybinding: None,
    },
    CommandEntry {
        label: "Show Status Panel",
        action: CommandAction::ShowStatus,
        keybinding: None,
    },
    CommandEntry {
        label: "Cycle Theme",
        action: CommandAction::ToggleTheme,
        keybinding: Some("Ctrl+Shift+T"),
    },
    CommandEntry {
        label: "Command Palette",
        action: CommandAction::CommandPalette,
        keybinding: Some("Ctrl+Shift+P"),
    },
];

pub fn titlebar_menu_entries(menu: TitlebarMenu) -> MenuEntries {
    match menu {
        TitlebarMenu::File => MenuEntries::WithSeparators(FILE_MENU),
        TitlebarMenu::Edit => MenuEntries::Flat(EDIT_MENU),
        TitlebarMenu::Selection => MenuEntries::Flat(SELECTION_MENU),
        TitlebarMenu::View => MenuEntries::Flat(VIEW_MENU),
    }
}

pub enum MenuEntries {
    WithSeparators(&'static [MenuEntry]),
    Flat(&'static [CommandEntry]),
}

pub fn command_palette_entries() -> Vec<CommandEntry> {
    let mut entries = Vec::new();
    for entry in FILE_MENU {
        if let MenuEntry::Item(item) = entry {
            entries.push(*item);
        }
    }
    entries.extend(EDIT_MENU.iter().copied());
    entries.extend(SELECTION_MENU.iter().copied());
    entries.extend(VIEW_MENU.iter().copied());
    entries
}
