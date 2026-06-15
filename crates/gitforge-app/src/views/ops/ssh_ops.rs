use gpui::Context;

use crate::views::app::GitForgeApp;

impl GitForgeApp {
    pub fn load_ssh_state(&mut self) {
        self.ssh_keys = gitforge_git::ssh::list_ssh_keys().unwrap_or_default();
        self.ssh_agent_status = Some(gitforge_git::ssh::check_ssh_agent());
    }

    pub fn generate_ssh_key(&mut self, key_type: String, email: String, cx: &mut Context<Self>) {
        self.run_blocking_op_returning(
            "SSH key generation",
            cx,
            move || gitforge_git::ssh::generate_ssh_key(&key_type, &email, None, None),
            |this, _path, cx| {
                this.load_ssh_state();
                this.repo_session.remote_status = "SSH key generated successfully".to_string();
                cx.notify();
            },
            |this, _cx| {
                this.repo_session.remote_status.clear();
            },
        );
    }

    pub fn test_ssh_connection(&mut self, host: String, cx: &mut Context<Self>) {
        let host_display = host.clone();
        self.repo_session.remote_status = format!("Testing SSH connection to {}...", host);
        cx.notify();

        self.run_blocking_op_returning(
            "SSH test",
            cx,
            move || gitforge_git::ssh::test_ssh_connection(&host),
            move |this, msg, cx| {
                this.repo_session.remote_status = format!("SSH test {}: {}", host_display, msg);
                cx.notify();
            },
            |this, _cx| {
                this.repo_session.remote_status.clear();
            },
        );
    }
}
