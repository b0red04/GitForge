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
                        this.remote_status = "SSH key generated successfully".to_string();
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("SSH key generation failed: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("SSH key generation error: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn delete_ssh_key(&mut self, key_name: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result =
                tokio::task::spawn_blocking(move || gitforge_git::ssh::delete_ssh_key(&key_name))
                    .await;

            match result {
                Ok(Ok(())) => {
                    this.update(cx, |this, cx| {
                        this.load_ssh_state();
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    tracing::error!("Failed to delete SSH key: {}", e);
                }
                Err(e) => {
                    tracing::error!("SSH key delete task panicked: {}", e);
                }
            }
        })
        .detach();
    }

    pub fn add_key_to_agent(&mut self, key_name: String, cx: &mut Context<Self>) {
        let key_name_display = key_name.clone();
        cx.spawn(async move |this, cx| {
            let kn = key_name;
            let kn_display = key_name_display;
            let result =
                tokio::task::spawn_blocking(move || gitforge_git::ssh::add_key_to_agent(&kn)).await;

            match result {
                Ok(Ok(())) => {
                    this.update(cx, |this, cx| {
                        this.load_ssh_state();
                        this.remote_status = format!("Key {} added to ssh-agent", kn_display);
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("Failed to add key to agent: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("ssh-add error: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn remove_key_from_agent(&mut self, key_name: String, cx: &mut Context<Self>) {
        let key_name_display = key_name.clone();
        cx.spawn(async move |this, cx| {
            let kn = key_name;
            let kn_display = key_name_display;
            let result =
                tokio::task::spawn_blocking(move || gitforge_git::ssh::remove_key_from_agent(&kn))
                    .await;

            match result {
                Ok(Ok(())) => {
                    this.update(cx, |this, cx| {
                        this.load_ssh_state();
                        this.remote_status = format!("Key {} removed from ssh-agent", kn_display);
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("Failed to remove key from agent: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("ssh-add error: {}", e);
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
        self.remote_status = format!("Testing SSH connection to {}...", host);
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
                        this.remote_status = format!("SSH test {}: {}", display, msg);
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("SSH test failed: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("SSH test error: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }
}
