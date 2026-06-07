use gpui::Context;

use crate::views::app::GitForgeApp;

impl GitForgeApp {
    pub fn open_in_editor(&mut self, path: std::path::PathBuf, _cx: &mut Context<Self>) {
        let cmd = &self.settings.tools.editor_command;
        let _ = std::process::Command::new(cmd).arg(&path).spawn();
    }

    pub fn open_in_terminal(&mut self, path: std::path::PathBuf, _cx: &mut Context<Self>) {
        let cmd = &self.settings.tools.terminal_command;
        let _ = std::process::Command::new(cmd)
            .arg("--working-directory")
            .arg(&path)
            .spawn()
            .or_else(|_| {
                std::process::Command::new(cmd)
                    .arg("--dir")
                    .arg(&path)
                    .spawn()
            })
            .or_else(|_| std::process::Command::new(cmd).current_dir(&path).spawn())
            .ok();
    }
}
