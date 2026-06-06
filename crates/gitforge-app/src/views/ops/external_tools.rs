use gpui::Context;

use crate::views::app::GitForgeApp;
use super::super::settings::CustomCommand;

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

    pub fn open_file_in_editor(
        &mut self,
        file_path: String,
        line: Option<usize>,
        _cx: &mut Context<Self>,
    ) {
        let cmd = &self.settings.tools.editor_command;
        let path = std::path::PathBuf::from(&file_path);
        let formatted = match line {
            Some(l) => format!("{}:{}", file_path, l),
            None => file_path.clone(),
        };
        let _ = std::process::Command::new(cmd)
            .arg(&formatted)
            .spawn()
            .or_else(|_| {
                std::process::Command::new(cmd)
                    .arg("+")
                    .arg(line.unwrap_or(1).to_string())
                    .arg(&file_path)
                    .spawn()
            })
            .or_else(|_| std::process::Command::new(cmd).arg(&path).spawn())
            .ok();
    }

    pub fn open_diff_tool(&mut self, old_path: &str, new_path: &str, _cx: &mut Context<Self>) {
        let tool = &self.settings.tools.diff_tool;
        if tool.is_empty() {
            return;
        }
        let _ = std::process::Command::new(tool)
            .arg(old_path)
            .arg(new_path)
            .spawn();
    }

    pub fn open_merge_tool(&mut self, file_path: &str, _cx: &mut Context<Self>) {
        let tool = &self.settings.tools.merge_tool;
        if tool.is_empty() {
            let _ = std::process::Command::new("git")
                .args(["mergetool", file_path])
                .spawn();
            return;
        }
        let _ = std::process::Command::new(tool).arg(file_path).spawn();
    }

    pub fn run_custom_command(
        &mut self,
        command: &CustomCommand,
        repo_path: &std::path::Path,
        file: Option<&str>,
        line: Option<usize>,
        commit: Option<&str>,
    ) {
        let mut cmd_str = command.command.clone();
        if let Some(f) = file {
            cmd_str = cmd_str.replace("{file}", f);
        }
        if let Some(l) = line {
            cmd_str = cmd_str.replace("{line}", &l.to_string());
        }
        if let Some(c) = commit {
            cmd_str = cmd_str.replace("{commit}", c);
        }
        cmd_str = cmd_str.replace("{repo}", &repo_path.to_string_lossy());
        let _ = std::process::Command::new("sh")
            .args(["-c", &cmd_str])
            .current_dir(repo_path)
            .spawn();
    }
}
