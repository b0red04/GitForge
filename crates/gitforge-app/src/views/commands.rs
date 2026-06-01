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

const FILE_MENU: &[CommandEntry] = &[
    CommandEntry {
        label: "Open Repository",
        action: "open_repository",
        keybinding: Some("Ctrl+O"),
    },
    CommandEntry {
        label: "Clone Repository",
        action: "clone",
        keybinding: None,
    },
    CommandEntry {
        label: "Clone from GitHub",
        action: "clone_github",
        keybinding: None,
    },
    CommandEntry {
        label: "Clone from GitLab",
        action: "clone_gitlab",
        keybinding: None,
    },
    CommandEntry {
        label: "Add Remote",
        action: "add_remote",
        keybinding: None,
    },
    CommandEntry {
        label: "Open in Browser",
        action: "open_browser",
        keybinding: None,
    },
    CommandEntry {
        label: "Open in Editor",
        action: "open_editor",
        keybinding: None,
    },
    CommandEntry {
        label: "Open in Terminal",
        action: "open_terminal",
        keybinding: None,
    },
    CommandEntry {
        label: "Create Worktree",
        action: "worktree",
        keybinding: None,
    },
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
        label: "Toggle Theme",
        action: "toggle_theme",
        keybinding: Some("Ctrl+Shift+T"),
    },
    CommandEntry {
        label: "Command Palette",
        action: "command_palette",
        keybinding: Some("Ctrl+Shift+P"),
    },
];

pub fn titlebar_menu_entries(menu: TitlebarMenu) -> &'static [CommandEntry] {
    match menu {
        TitlebarMenu::File => FILE_MENU,
        TitlebarMenu::Edit => EDIT_MENU,
        TitlebarMenu::Selection => SELECTION_MENU,
        TitlebarMenu::View => VIEW_MENU,
    }
}

pub fn command_palette_entries() -> Vec<CommandEntry> {
    FILE_MENU
        .iter()
        .chain(EDIT_MENU.iter())
        .chain(SELECTION_MENU.iter())
        .chain(VIEW_MENU.iter())
        .copied()
        .collect()
}
