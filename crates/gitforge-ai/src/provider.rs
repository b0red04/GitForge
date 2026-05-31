use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn generate(&self, prompt: &str, system: Option<&str>) -> Result<String>;

    async fn generate_commit_message(&self, diff: &str, conventional: bool, tone: &str) -> Result<String> {
        let prompt = crate::prompt::build_commit_message_prompt(diff, conventional, tone);
        let system = Some("You are an expert at writing git commit messages. Output only the commit message, nothing else.");
        self.generate(&prompt, system).await
    }

    async fn generate_commit_messages(&self, diff: &str, conventional: bool, count: usize) -> Result<Vec<String>> {
        let prompt = crate::prompt::build_multi_commit_message_prompt(diff, conventional, count);
        let system = Some("You are an expert at writing git commit messages. Output only the commit messages separated by ---, nothing else.");
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

    async fn summarize_diff(&self, diff: &str) -> Result<String> {
        let prompt = crate::prompt::build_diff_summary_prompt(diff);
        let system = Some("You are a code review assistant. Provide a brief, helpful summary of changes.");
        self.generate(&prompt, system).await
    }
}
