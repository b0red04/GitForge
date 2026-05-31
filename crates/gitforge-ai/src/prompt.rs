pub fn build_commit_message_prompt(diff: &str, conventional: bool, tone: &str) -> String {
    let format_instruction = if conventional {
        "Write a conventional commit message (type(scope): description). The type should be one of: feat, fix, docs, style, refactor, perf, test, chore, build, ci."
    } else {
        "Write a clear commit message in imperative mood."
    };

    let tone_instruction = match tone {
        "concise" => "Keep the message very short — one line, under 50 characters if possible.",
        "detailed" => "Provide a detailed explanation with a summary line and a multi-paragraph body describing all significant changes.",
        _ => "The first line should be a short summary (50 chars or less). If needed, add a blank line followed by a detailed explanation wrapping at 72 chars.",
    };

    format!(
        "Analyze the following diff and generate an appropriate commit message.\n\n\
         Rules:\n\
         - {format_instruction}\n\
         - {tone_instruction}\n\
         - Do not mention the commit message itself in the output\n\
         - Do not add quotes around the message\n\
         - Only output the commit message, nothing else\n\n\
         Diff:\n```\n{diff}\n```"
    )
}

pub fn build_multi_commit_message_prompt(diff: &str, conventional: bool, count: usize) -> String {
    let format_instruction = if conventional {
        "Each message should follow conventional commit format (type(scope): description)."
    } else {
        "Each message should use imperative mood."
    };

    format!(
        "Analyze the following diff and generate {count} different commit message options.\n\n\
         Rules:\n\
         - {format_instruction}\n\
         - Vary the style: make one concise, one detailed, and one creative\n\
         - Separate each message with a line containing only '---'\n\
         - Do not number the messages\n\
         - Do not add quotes around messages\n\
         - Do not include any other text\n\n\
         Diff:\n```\n{diff}\n```"
    )
}

pub fn build_diff_summary_prompt(diff: &str) -> String {
    format!(
        "Provide a brief summary (2-3 sentences) of what changed in this diff. \
         Focus on the intent and impact, not line-by-line details.\n\n\
         Diff:\n```\n{diff}\n```"
    )
}
