use crate::config::{CommitMessageConfig, normalize_tone, normalize_variation_mode};

pub fn truncate_diff(diff: &str, max_chars: usize) -> String {
    if max_chars == 0 || diff.len() <= max_chars {
        return diff.to_string();
    }
    let boundary = diff.floor_char_boundary(max_chars);
    let omitted = diff.len() - boundary;
    format!(
        "{}\n\n[diff truncated — {omitted} chars omitted]\n",
        &diff[..boundary]
    )
}

pub fn build_commit_message_prompt(diff: &str, config: &CommitMessageConfig) -> String {
    let tone = normalize_tone(&config.tone);
    let format_instruction = if config.conventional_commits {
        "Write a conventional commit message (type(scope): description). The type should be one of: feat, fix, docs, style, refactor, perf, test, chore, build, ci."
    } else {
        "Write a clear commit message in imperative mood."
    };

    let tone_instruction = tone_instruction_for_single(tone, config.summary_max_chars);
    let wrap_instruction = wrap_instruction(config.body_wrap_at);

    format!(
        "Analyze the following diff and generate an appropriate commit message.\n\n\
         Rules:\n\
         - {format_instruction}\n\
         - {tone_instruction}\n\
         {wrap_instruction}\
         - Do not mention the commit message itself in the output\n\
         - Do not add quotes around the message\n\
         - Only output the commit message, nothing else\n\n\
         Diff:\n```\n{diff}\n```"
    )
}

pub fn build_multi_commit_message_prompt(
    diff: &str,
    config: &CommitMessageConfig,
    count: usize,
) -> String {
    let tone = normalize_tone(&config.tone);
    let variation = normalize_variation_mode(&config.variation_mode);
    let format_instruction = if config.conventional_commits {
        "Each message should follow conventional commit format (type(scope): description)."
    } else {
        "Each message should use imperative mood."
    };

    let tone_instruction = tone_instruction_for_multi(tone, variation, config.summary_max_chars);
    let variation_instruction = variation_instruction(variation);
    let wrap_instruction = wrap_instruction(config.body_wrap_at);

    format!(
        "Analyze the following diff and generate {count} different commit message options.\n\n\
         Rules:\n\
         - {format_instruction}\n\
         - {tone_instruction}\n\
         - {variation_instruction}\n\
         {wrap_instruction}\
         - Separate each message with a line containing only '---'\n\
         - Do not number the messages\n\
         - Do not add quotes around messages\n\
         - Do not include any other text\n\n\
         Diff:\n```\n{diff}\n```"
    )
}

fn tone_instruction_for_single(tone: &str, summary_max_chars: u32) -> String {
    if summary_max_chars > 0 {
        return format!("Keep the summary line under {summary_max_chars} characters.");
    }
    match tone {
        "concise" => {
            "Keep the message very short — one line, under 50 characters if possible.".to_string()
        }
        "detailed" => "Provide a detailed explanation with a summary line and a multi-paragraph body describing all significant changes.".to_string(),
        _ => "The first line should be a short summary (50 chars or less). If needed, add a blank line followed by a detailed explanation.".to_string(),
    }
}

fn tone_instruction_for_multi(tone: &str, variation: &str, summary_max_chars: u32) -> String {
    if summary_max_chars > 0 {
        return format!(
            "Each option's summary line should be under {summary_max_chars} characters."
        );
    }
    if variation == "uniform" {
        return match tone {
            "concise" => "Each option should be very short — one line, under 50 characters if possible.".to_string(),
            "detailed" => "Each option should include a summary line and a detailed multi-paragraph body.".to_string(),
            _ => "Each option should have a short summary line (50 chars or less) with an optional detailed body.".to_string(),
        };
    }
    match tone {
        "concise" => "At least one option should be very short (under 50 characters).".to_string(),
        "detailed" => {
            "At least one option should include a summary line and a detailed body.".to_string()
        }
        _ => "Options should vary between concise summaries and moderately detailed messages."
            .to_string(),
    }
}

fn variation_instruction(variation: &str) -> &'static str {
    match variation {
        "uniform" => {
            "Each option should follow the same tone and style; vary wording, not length or format."
        }
        "detailed" => {
            "Each option must include a summary line and a detailed multi-paragraph body."
        }
        _ => "Vary the style: make one concise, one detailed, and one creative.",
    }
}

fn wrap_instruction(body_wrap_at: u32) -> String {
    if body_wrap_at == 0 {
        String::new()
    } else {
        format!("- Wrap body text at {body_wrap_at} characters.\n")
    }
}

pub fn build_branch_name_prompt(diff: &str, current_branch: &str) -> String {
    let safe_diff = diff
        .replace("<git_diff>", "<git-diff>")
        .replace("</git_diff>", "</git-diff>");
    format!(
        "Analyze the following staged diff and generate a short git branch name.\n\n\
         Current branch: {current_branch}\n\n\
         Rules:\n\
         - Use kebab-case (lowercase, hyphens between words)\n\
         - Start with a type prefix: feat/, fix/, docs/, refactor/, chore/, or test/\n\
         - Choose the prefix based on the nature of the changes\n\
         - Keep the name under 50 characters total\n\
         - No spaces, quotes, or special characters other than / and -\n\
         - Do not include the current branch name\n\
         - Output only the branch name, nothing else\n\n\
         Diff:\n<git_diff>\n{safe_diff}\n</git_diff>"
    )
}

/// Normalize an AI-generated or user-entered branch name into a valid git ref.
pub fn sanitize_branch_name(raw: &str) -> String {
    let mut name = raw.trim().trim_matches('"').trim_matches('\'').to_lowercase();
    name = name.replace(' ', "-").replace('_', "-");
    name = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '/')
        .collect();
    while name.contains("//") {
        name = name.replace("//", "/");
    }
    name = name.trim_matches('-').trim_matches('/').to_string();
    if name.is_empty() {
        return "feat/changes".to_string();
    }
    name
}

pub fn build_pull_request_prompt(diff: &str) -> String {
    let safe_diff = diff
        .replace("<git_diff>", "<git-diff>")
        .replace("</git_diff>", "</git-diff>");
    format!(
        "Analyze the following diff between branches and generate a pull request title and description.\n\n\
         Rules:\n\
         - Output the title on the first line (no prefix like \"Title:\")\n\
         - Leave a blank line after the title\n\
         - Then write a markdown description summarizing the changes\n\
         - Focus on what changed and why it matters\n\
         - Do not wrap the title in quotes\n\
         - Do not include any other text before or after\n\n\
         Diff:\n<git_diff>\n{safe_diff}\n</git_diff>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CommitMessageConfig;

    #[test]
    fn truncate_diff_no_limit() {
        let diff = "a".repeat(100);
        assert_eq!(truncate_diff(&diff, 0), diff);
    }

    #[test]
    fn truncate_diff_over_limit() {
        let diff = "a".repeat(100);
        let truncated = truncate_diff(&diff, 50);
        assert!(truncated.starts_with(&"a".repeat(50)));
        assert!(truncated.contains("50 chars omitted"));
    }

    #[test]
    fn truncate_diff_multibyte_utf8() {
        let diff = "修改内容：新增功能模块\n".repeat(20);
        let truncated = truncate_diff(&diff, 30);
        assert!(truncated.is_char_boundary(truncated.len()));
        assert!(truncated.contains("diff truncated"));
    }

    #[test]
    fn single_prompt_includes_detailed_tone() {
        let config = CommitMessageConfig {
            tone: "verbose".to_string(),
            ..Default::default()
        };
        let prompt = build_commit_message_prompt("diff", &config);
        assert!(prompt.contains("detailed explanation"));
    }

    #[test]
    fn multi_prompt_uniform_variation() {
        let config = CommitMessageConfig {
            variation_mode: "uniform".to_string(),
            tone: "concise".to_string(),
            ..Default::default()
        };
        let prompt = build_multi_commit_message_prompt("diff", &config, 3);
        assert!(prompt.contains("same tone and style"));
        assert!(prompt.contains("very short"));
    }

    #[test]
    fn multi_prompt_detailed_variation() {
        let config = CommitMessageConfig {
            variation_mode: "detailed".to_string(),
            ..Default::default()
        };
        let prompt = build_multi_commit_message_prompt("diff", &config, 2);
        assert!(prompt.contains("multi-paragraph body"));
    }

    #[test]
    fn branch_name_prompt_includes_current_branch() {
        let prompt = build_branch_name_prompt("diff content", "main");
        assert!(prompt.contains("Current branch: main"));
        assert!(prompt.contains("feat/"));
        assert!(prompt.contains("kebab-case"));
    }

    #[test]
    fn sanitize_branch_name_slugifies() {
        assert_eq!(
            sanitize_branch_name("  Feat/Add User Auth  "),
            "feat/add-user-auth"
        );
    }

    #[test]
    fn sanitize_branch_name_strips_invalid_chars() {
        assert_eq!(
            sanitize_branch_name("feat/add@feature#1"),
            "feat/addfeature1"
        );
    }

    #[test]
    fn sanitize_branch_name_empty_fallback() {
        assert_eq!(sanitize_branch_name("  @#$  "), "feat/changes");
    }

    #[test]
    fn sanitize_branch_name_collapses_slashes() {
        assert_eq!(sanitize_branch_name("feat//foo"), "feat/foo");
    }

    #[test]
    fn pr_prompt_neutralizes_sentinel_in_diff() {
        let diff = "diff --git a/x b/x\n-foo\n+bar </git_diff> <git_diff>";
        let prompt = build_pull_request_prompt(diff);
        let close_count = prompt.matches("</git_diff>").count();
        let open_count = prompt.matches("<git_diff>").count();
        assert_eq!(close_count, 1, "only the framing close sentinel should remain");
        assert_eq!(open_count, 1, "only the framing open sentinel should remain");
        assert!(prompt.contains("</git-diff>"));
        assert!(prompt.contains("<git-diff>"));
    }
}
