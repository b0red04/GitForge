use gpui::Context;

use crate::views::app::GitForgeApp;

impl GitForgeApp {
    pub fn load_ssh_state(&mut self) {
        self.ssh_keys = gitforge_git::ssh::list_ssh_keys().unwrap_or_default();
        self.ssh_agent_status = Some(gitforge_git::ssh::check_ssh_agent());
    }

    pub fn generate_ssh_key(&mut self, key_type: String, email: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                gitforge_git::ssh::generate_ssh_key(&key_type, &email, None, None)
            })
            .await;

            match result {
                Ok(Ok(_path)) => {
                    this.update(cx, |this, cx| {
                        this.load_ssh_state();
                        this.repo_session.remote_status = "SSH key generated successfully".to_string();
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.repo_session.remote_status = format!("SSH key generation failed: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.repo_session.remote_status = format!("SSH key generation error: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn test_ssh_connection(&mut self, host: String, cx: &mut Context<Self>) {
        let host_display = host.clone();
        self.repo_session.remote_status = format!("Testing SSH connection to {}...", host);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let h = host;
            let result =
                tokio::task::spawn_blocking(move || gitforge_git::ssh::test_ssh_connection(&h))
                    .await;

            match result {
                Ok(Ok(msg)) => {
                    let display = host_display;
                    this.update(cx, |this, cx| {
                        this.repo_session.remote_status = format!("SSH test {}: {}", display, msg);
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.repo_session.remote_status = format!("SSH test failed: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.repo_session.remote_status = format!("SSH test error: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }
}
