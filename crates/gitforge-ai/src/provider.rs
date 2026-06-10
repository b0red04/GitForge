use anyhow::Result;
use async_trait::async_trait;

use crate::config::CommitMessageConfig;
use crate::prompt::{
    build_commit_message_prompt, build_multi_commit_message_prompt, truncate_diff,
};

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn generate(&self, prompt: &str, system: Option<&str>) -> Result<String>;

    async fn generate_commit_messages(
        &self,
        diff: &str,
        config: &CommitMessageConfig,
    ) -> Result<Vec<String>> {
        let diff = truncate_diff(diff, config.max_diff_chars);
        let count = config.options_count();

        if count == 1 {
            let prompt = build_commit_message_prompt(&diff, config);
            let system = Some(
                "You are an expert at writing git commit messages. Output only the commit message, nothing else.",
            );
            let message = self.generate(&prompt, system).await?;
            return Ok(vec![message.trim().to_string()]);
        }

        let prompt = build_multi_commit_message_prompt(&diff, config, count);
        let system = Some(
            "You are an expert at writing git commit messages. Output only the commit messages separated by ---, nothing else.",
        );
        let raw = self.generate(&prompt, system).await?;
        let messages: Vec<String> = raw
            .split("\n---\n")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if messages.is_empty() {
            Ok(vec![raw.trim().to_string()])
        } else {
            Ok(messages)
        }
    }
}
