use crate::commit::CommitInfo;
use crate::error::{GitError, GitResult};

/// Action for a single commit in an interactive rebase plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RebaseAction {
    #[default]
    Pick,
    Squash,
    Fixup,
    Reword,
    Drop,
    Edit,
}

impl RebaseAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pick => "pick",
            Self::Squash => "squash",
            Self::Fixup => "fixup",
            Self::Reword => "reword",
            Self::Drop => "drop",
            Self::Edit => "edit",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pick => "Pick",
            Self::Squash => "Squash",
            Self::Fixup => "Fixup",
            Self::Reword => "Reword",
            Self::Drop => "Drop",
            Self::Edit => "Edit",
        }
    }

    /// Short UI hint shown in the squash wizard action menu.
    pub fn hint(self) -> &'static str {
        match self {
            Self::Pick => "Keep this commit",
            Self::Squash => "Merge into the commit above",
            Self::Fixup => "Merge above, discard message",
            Self::Reword => "Keep changes, edit message",
            Self::Drop => "Remove this commit",
            Self::Edit => "Pause to amend this commit",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Pick,
            Self::Squash,
            Self::Fixup,
            Self::Reword,
            Self::Drop,
            Self::Edit,
        ]
    }

    /// Actions offered in the squash wizard for a commit at `entry_index` (oldest = 0).
    pub fn available_for_entry(entry_index: usize) -> Vec<Self> {
        Self::all()
            .iter()
            .copied()
            .filter(|&action| {
                entry_index > 0 || !matches!(action, Self::Squash | Self::Fixup)
            })
            .collect()
    }
}

/// One commit in an interactive rebase plan (oldest → newest).
#[derive(Debug, Clone)]
pub struct RebasePlanEntry {
    pub sha: String,
    pub short_id: String,
    pub summary: String,
    pub action: RebaseAction,
    /// Custom message for `Reword`, or combined message for a trailing `Squash` group.
    pub message: Option<String>,
}

impl RebasePlanEntry {
    pub fn from_commit(commit: &CommitInfo, action: RebaseAction) -> Self {
        Self {
            sha: commit.id.clone(),
            short_id: commit.short_id.clone(),
            summary: commit.summary.clone(),
            action,
            message: None,
        }
    }
}

/// Interactive rebase plan: replay `entries` on top of `onto`.
#[derive(Debug, Clone)]
pub struct RebasePlan {
    pub onto: String,
    pub entries: Vec<RebasePlanEntry>,
    /// Combined message used when squashing multiple commits into one group.
    pub combined_message: Option<String>,
}

impl RebasePlan {
    pub fn from_commits(onto: impl Into<String>, commits: &[CommitInfo]) -> Self {
        Self {
            onto: onto.into(),
            entries: commits
                .iter()
                .map(|c| RebasePlanEntry::from_commit(c, RebaseAction::Pick))
                .collect(),
            combined_message: None,
        }
    }

    /// First commit `pick`, remaining `squash` — common PR cleanup path.
    pub fn squash_all_into_one(
        onto: impl Into<String>,
        commits: &[CommitInfo],
        message: impl Into<String>,
    ) -> Self {
        let mut entries: Vec<RebasePlanEntry> = commits
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let action = if i == 0 {
                    RebaseAction::Pick
                } else {
                    RebaseAction::Squash
                };
                RebasePlanEntry::from_commit(c, action)
            })
            .collect();
        let msg = message.into();
        if let Some(last) = entries.last_mut() {
            last.message = Some(msg.clone());
        }
        Self {
            onto: onto.into(),
            entries,
            combined_message: Some(msg),
        }
    }

    pub fn validate(&self) -> GitResult<()> {
        if self.entries.is_empty() {
            return Err(GitError::OperationFailed(
                "Rebase plan has no commits".into(),
            ));
        }

        let mut has_non_drop = false;
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.action != RebaseAction::Drop {
                has_non_drop = true;
            }
            match entry.action {
                RebaseAction::Squash | RebaseAction::Fixup => {
                    // A squash/fixup must follow a continuation-capable entry
                    // (Pick, Edit, Squash, or Fixup). The first entry has no
                    // predecessor, and Drop/Reword break the adjacency the
                    // editor-message queue relies on.
                    let valid_prev = i > 0
                        && matches!(
                            self.entries[i - 1].action,
                            RebaseAction::Pick
                                | RebaseAction::Edit
                                | RebaseAction::Squash
                                | RebaseAction::Fixup
                        );
                    if !valid_prev {
                        return Err(GitError::OperationFailed(
                            "Squash/Fixup must immediately follow a Pick, Edit, or another \
                             Squash/Fixup"
                                .into(),
                        ));
                    }
                }
                _ => {}
            }
        }

        if !has_non_drop {
            return Err(GitError::OperationFailed(
                "Rebase plan must keep at least one commit".into(),
            ));
        }

        Ok(())
    }

    /// Git rebase todo file lines (`pick abc1234 subject`).
    pub fn todo_lines(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|e| {
                format!(
                    "{} {} {}",
                    e.action.as_str(),
                    &e.sha[..e.sha.len().min(7)],
                    e.summary
                )
            })
            .collect()
    }

    /// Messages for `GIT_EDITOR` invocations, in the order git requests them.
    pub fn editor_message_queue(&self) -> Vec<String> {
        let mut queue = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            let entry = &self.entries[i];
            match entry.action {
                RebaseAction::Reword => {
                    queue.push(
                        entry
                            .message
                            .clone()
                            .unwrap_or_else(|| entry.summary.clone()),
                    );
                    i += 1;
                }
                RebaseAction::Pick | RebaseAction::Edit => {
                    let start = i;
                    let mut j = i + 1;
                    while j < self.entries.len()
                        && matches!(
                            self.entries[j].action,
                            RebaseAction::Squash | RebaseAction::Fixup
                        )
                    {
                        j += 1;
                    }
                    let group = &self.entries[start..j];
                    if group
                        .iter()
                        .any(|e| e.action == RebaseAction::Squash)
                    {
                        let msg = self.combined_message.clone().or_else(|| {
                            group
                                .iter()
                                .rev()
                                .find(|e| e.message.is_some())
                                .and_then(|e| e.message.clone())
                        }).unwrap_or_else(|| {
                            group
                                .iter()
                                .map(|e| e.summary.as_str())
                                .collect::<Vec<_>>()
                                .join("\n\n")
                        });
                        queue.push(msg);
                    }
                    i = j;
                }
                RebaseAction::Squash | RebaseAction::Fixup => {
                    // Handled as part of the preceding pick group.
                    i += 1;
                }
                RebaseAction::Drop => {
                    i += 1;
                }
            }
        }
        queue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_commit(id: &str, summary: &str) -> CommitInfo {
        CommitInfo {
            id: id.to_string(),
            short_id: id[..7].to_string(),
            message: summary.to_string(),
            summary: summary.to_string(),
            author_name: "a".into(),
            author_email: "a@b.c".into(),
            author_date: Utc::now(),
            committer_name: "a".into(),
            committer_email: "a@b.c".into(),
            committer_date: Utc::now(),
            parent_ids: vec!["parent".into()],
        }
    }

    #[test]
    fn squash_all_into_one_plan_shape() {
        let commits = vec![
            sample_commit("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "first"),
            sample_commit("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "second"),
            sample_commit("cccccccccccccccccccccccccccccccccccccccc", "third"),
        ];
        let plan = RebasePlan::squash_all_into_one("base123", &commits, "combined");
        assert_eq!(plan.entries.len(), 3);
        assert_eq!(plan.entries[0].action, RebaseAction::Pick);
        assert_eq!(plan.entries[1].action, RebaseAction::Squash);
        assert_eq!(plan.entries[2].action, RebaseAction::Squash);
        assert_eq!(plan.combined_message.as_deref(), Some("combined"));
    }

    #[test]
    fn todo_lines_use_short_sha() {
        let commits = vec![sample_commit(
            "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
            "subject",
        )];
        let plan = RebasePlan::from_commits("base", &commits);
        assert_eq!(plan.todo_lines(), vec!["pick deadbee subject"]);
    }

    #[test]
    fn editor_queue_for_squash_all() {
        let commits = vec![
            sample_commit("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "a"),
            sample_commit("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "b"),
        ];
        let plan = RebasePlan::squash_all_into_one("base", &commits, "squashed");
        assert_eq!(plan.editor_message_queue(), vec!["squashed"]);
    }

    #[test]
    fn validate_rejects_squash_first() {
        let mut plan = RebasePlan::from_commits(
            "base",
            &[sample_commit(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "only",
            )],
        );
        plan.entries[0].action = RebaseAction::Squash;
        assert!(plan.validate().is_err());
    }

    #[test]
    fn validate_rejects_non_contiguous_squash() {
        // Pick, Drop, Squash — the squash no longer follows a continuation.
        let mut plan = RebasePlan::from_commits(
            "base",
            &[
                sample_commit("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "a"),
                sample_commit("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "b"),
                sample_commit("cccccccccccccccccccccccccccccccccccccccc", "c"),
            ],
        );
        plan.entries[1].action = RebaseAction::Drop;
        plan.entries[2].action = RebaseAction::Squash;
        assert!(plan.validate().is_err());
    }
}
