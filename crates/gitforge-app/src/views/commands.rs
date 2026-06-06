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

#[derive(Debug, Clone, Copy)]
pub struct CommandEntry {
    pub label: &'static str,
    pub action: &'static str,
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
        action: "new_tab",
        keybinding: Some("Ctrl+T"),
    }),
    MenuEntry::Item(CommandEntry {
        label: "Close Tab",
        action: "close_tab",
        keybinding: Some("Ctrl+W"),
    }),
    MenuEntry::Item(CommandEntry {
        label: "Reopen Closed Tab",
        action: "reopen_closed_tab",
        keybinding: None,
    }),
    MenuEntry::Separator,
    MenuEntry::Item(CommandEntry {
        label: "Clone Repo...",
        action: "clone",
        keybinding: None,
    }),
    MenuEntry::Item(CommandEntry {
        label: "Init Repo...",
        action: "init_repo",
        keybinding: Some("Ctrl+I"),
    }),
    MenuEntry::Item(CommandEntry {
        label: "Open Repo...",
        action: "open_repository",
        keybinding: Some("Ctrl+O"),
    }),
    MenuEntry::Item(CommandEntry {
        label: "Open Repo Management",
        action: "repo_management",
        keybinding: Some("Alt+Ctrl+O"),
    }),
    MenuEntry::Item(CommandEntry {
        label: "Clone from GitHub",
        action: "clone_github",
        keybinding: None,
    }),
    MenuEntry::Item(CommandEntry {
        label: "Clone from GitLab",
        action: "clone_gitlab",
        keybinding: None,
    }),
    MenuEntry::Item(CommandEntry {
        label: "Add Remote",
        action: "add_remote",
        keybinding: None,
    }),
    MenuEntry::Separator,
    MenuEntry::Item(CommandEntry {
        label: "Open Repo in External Editor",
        action: "open_editor",
        keybinding: Some("Ctrl+Shift+E"),
    }),
    MenuEntry::Item(CommandEntry {
        label: "Open External Terminal",
        action: "open_terminal",
        keybinding: Some("Alt+T"),
    }),
    MenuEntry::Item(CommandEntry {
        label: "Open in File Manager",
        action: "open_file_manager",
        keybinding: Some("Alt+O"),
    }),
    MenuEntry::Item(CommandEntry {
        label: "Open in Browser",
        action: "open_browser",
        keybinding: None,
    }),
    MenuEntry::Item(CommandEntry {
        label: "Create Worktree",
        action: "worktree",
        keybinding: None,
    }),
    MenuEntry::Separator,
    MenuEntry::Item(CommandEntry {
        label: "Preferences...",
        action: "preferences",
        keybinding: Some("Ctrl+,"),
    }),
    MenuEntry::Separator,
    MenuEntry::Item(CommandEntry {
        label: "Sign into a Different Account",
        action: "accounts",
        keybinding: None,
    }),
    MenuEntry::Item(CommandEntry {
        label: "Quit GitForge",
        action: "quit",
        keybinding: Some("Ctrl+Q"),
    }),
];

const EDIT_MENU: &[CommandEntry] = &[
    CommandEntry {
        label: "Refresh Repository",
        action: "refresh",
        keybinding: None,
    },
    CommandEntry {
        label: "Create Branch",
        action: "create_branch",
        keybinding: Some("Ctrl+N"),
    },
    CommandEntry {
        label: "Stash Changes",
        action: "stash_push",
        keybinding: Some("Ctrl+Shift+S"),
    },
    CommandEntry {
        label: "Pop Stash",
        action: "stash_pop",
        keybinding: Some("Ctrl+Shift+O"),
    },
    CommandEntry {
        label: "Undo Last Commit",
        action: "soft_reset",
        keybinding: None,
    },
    CommandEntry {
        label: "AI Settings",
        action: "ai_settings",
        keybinding: None,
    },
    CommandEntry {
        label: "Manage Accounts",
        action: "accounts",
        keybinding: None,
    },
    CommandEntry {
        label: "Generate SSH Key",
        action: "ssh_key",
        keybinding: None,
    },
];

const SELECTION_MENU: &[CommandEntry] = &[
    CommandEntry {
        label: "Select Previous Commit",
        action: "select_prev",
        keybinding: Some("Up"),
    },
    CommandEntry {
        label: "Select Next Commit",
        action: "select_next",
        keybinding: Some("Down"),
    },
    CommandEntry {
        label: "View File at Commit",
        action: "view_file",
        keybinding: Some("Enter"),
    },
    CommandEntry {
        label: "Back to Diff",
        action: "back_to_diff",
        keybinding: Some("Escape"),
    },
];

const VIEW_MENU: &[CommandEntry] = &[
    CommandEntry {
        label: "Show History",
        action: "show_history",
        keybinding: None,
    },
    CommandEntry {
        label: "Show Status Panel",
        action: "show_status",
        keybinding: None,
    },
    CommandEntry {
        label: "Cycle Theme",
        action: "toggle_theme",
        keybinding: Some("Ctrl+Shift+T"),
    },
    CommandEntry {
        label: "Command Palette",
        action: "command_palette",
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
