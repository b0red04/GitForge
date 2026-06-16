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
                this.push_toast(
                    crate::views::toasts::ToastKind::Success,
                    "SSH key generated successfully".to_string(),
                    cx,
                );
            },
            |_, _| {},
        );
    }

    pub fn test_ssh_connection(&mut self, host: String, cx: &mut Context<Self>) {
        let host_display = host.clone();

        self.run_blocking_op_returning(
            "SSH test",
            cx,
            move || gitforge_git::ssh::test_ssh_connection(&host),
            move |this, msg, cx| {
                this.push_toast(
                    crate::views::toasts::ToastKind::Info,
                    format!("SSH test {}: {}", host_display, msg),
                    cx,
                );
            },
            |_, _| {},
        );
    }
}
