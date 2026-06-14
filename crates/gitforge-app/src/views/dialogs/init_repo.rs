use gitforge_git::{GitError, Repository};
use gpui::*;

use std::path::PathBuf;

use crate::views::app::GitForgeApp;

pub fn init_repository(_app: &mut GitForgeApp, parent: PathBuf, name: String, cx: &mut Context<GitForgeApp>) {
    let repo_path = parent.join(&name);
    cx.spawn(async move |this, cx| {
        let result = tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&repo_path)
                .map_err(|e| GitError::OperationFailed(e.to_string()))?;
            Repository::init_repo(&repo_path, false)?;
            Ok::<PathBuf, GitError>(repo_path)
        })
        .await;

        match result {
            Ok(Ok(path)) => {
                this.update(cx, |this, cx| {
                    this.open_or_activate_repo_tab(path, cx);
                })
                .ok();
            }
            Ok(Err(e)) => {
                this.update(cx, |this, cx| {
                    this.repo_session.last_error =
                        Some(format!("Failed to init repository: {}", e));
                    cx.notify();
                })
                .ok();
            }
            Err(e) => {
                this.update(cx, |this, cx| {
                    this.repo_session.last_error = Some(format!("Init task panicked: {}", e));
                    cx.notify();
                })
                .ok();
            }
        }
    })
    .detach();
}

pub fn spawn_init_repo_picker(_app: &mut GitForgeApp, cx: &mut Context<GitForgeApp>) {
    cx.spawn(async move |this, cx| {
        let path =
            cx.update(|_cx| rfd::AsyncFileDialog::new().set_title("Select Parent Directory"));
        let folder = match path {
            Ok(dialog) => dialog.pick_folder().await,
            Err(_) => None,
        };

        let Some(folder) = folder else {
            return;
        };

        let parent = std::path::PathBuf::from(folder.path());
        this.update(cx, |this, cx| {
            this.active_dialog = crate::views::app::AppDialog::InitRepo { parent };
            this.dialog_input.set_text("new-repo");
            cx.notify();
        })
        .ok();
    })
    .detach();
}
