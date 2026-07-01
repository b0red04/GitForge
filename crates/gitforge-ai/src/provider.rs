use crate::error::AiResult;
use async_trait::async_trait;

use crate::config::CommitMessageConfig;
use crate::prompt::{
    build_branch_name_prompt, build_commit_message_prompt, build_multi_commit_message_prompt,
    build_pull_request_prompt, sanitize_branch_name, sanitize_commit_message, truncate_diff,
};

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn generate(&self, prompt: &str, system: Option<&str>) -> AiResult<String>;

    async fn generate_commit_messages(
        &self,
        diff: &str,
        config: &CommitMessageConfig,
    ) -> AiResult<Vec<String>> {
        let diff = truncate_diff(diff, config.max_diff_chars);
        let count = config.options_count();

        if count == 1 {
            let prompt = build_commit_message_prompt(&diff, config);
            let system = Some(
                "You are an expert at writing git commit messages. Output only the commit message, nothing else.",
            );
            let message = self.generate(&prompt, system).await?;
            return Ok(vec![sanitize_commit_message(&message)]);
        }

        let prompt = build_multi_commit_message_prompt(&diff, config, count);
        let system = Some(
            "You are an expert at writing git commit messages. Output only the commit messages separated by ---, nothing else.",
        );
        let raw = self.generate(&prompt, system).await?;
        let messages: Vec<String> = raw
            .split("\n---\n")
            .map(sanitize_commit_message)
            .filter(|s| !s.is_empty())
            .collect();
        if messages.is_empty() {
            Ok(vec![sanitize_commit_message(&raw)])
        } else {
            Ok(messages)
        }
    }

    async fn generate_branch_name(
        &self,
        diff: &str,
        current_branch: &str,
        max_diff_chars: usize,
    ) -> AiResult<String> {
        let diff = truncate_diff(diff, max_diff_chars);
        let prompt = build_branch_name_prompt(&diff, current_branch);
        let system = Some(
            "You are an expert at naming git branches. Output only the branch name, nothing else.",
        );
        let raw = self.generate(&prompt, system).await?;
        Ok(sanitize_branch_name(&raw))
    }

    async fn generate_pull_request_content(
        &self,
        diff: &str,
        max_diff_chars: usize,
    ) -> AiResult<(String, String)> {
        let diff = truncate_diff(diff, max_diff_chars);
        let prompt = build_pull_request_prompt(&diff);
        let system = Some(
            "You are an expert at writing pull request titles and descriptions. \
             Output only the title on the first line, a blank line, then the markdown body.",
        );
        let raw = self.generate(&prompt, system).await?;
        let trimmed = raw.trim();
        if let Some((title, body)) = trimmed.split_once("\n\n") {
            Ok((title.trim().to_string(), body.trim().to_string()))
        } else if let Some((title, body)) = trimmed.split_once('\n') {
            Ok((title.trim().to_string(), body.trim().to_string()))
        } else {
            Ok((trimmed.to_string(), String::new()))
        }
    }
}
