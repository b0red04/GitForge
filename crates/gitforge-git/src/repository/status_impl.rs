use crate::error::{GitError, GitResult};
use crate::repository::Repository;
use crate::status::{FileEntry, FileStatus, RepoStatus};
use crate::diff_stat::untracked_line_count;

fn file_entry(
    path: String,
    old_path: Option<String>,
    status: FileStatus,
    staged: bool,
) -> FileEntry {
    FileEntry {
        path,
        old_path,
        status,
        staged,
        diff_stat: None,
    }
}

impl Repository {
    pub fn status(&self) -> GitResult<RepoStatus> {
        let mut result = RepoStatus::default();

        result.head_branch = self.head_branch()?;
        result.head_commit = self.head_commit()?.map(|c| c.short_id);

        let platform = self.repo.status(gix::progress::Discard)
            .map_err(|e| GitError::OperationFailed(e.to_string()))?
            .untracked_files(gix::status::UntrackedFiles::Files);

        let mut iter = platform.into_iter(vec![])
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;

        while let Some(item) = iter.next() {
            let item = item.map_err(|e| GitError::OperationFailed(e.to_string()))?;
            match &item {
                gix::status::Item::TreeIndex(change) => {
                    let (location, _index, _mode, _id) = change.fields();
                    let path_str = location.to_string();
                    let entry = match change {
                        gix_diff::index::ChangeRef::Addition { .. } => {
                            file_entry(path_str, None, FileStatus::Added, true)
                        }
                        gix_diff::index::ChangeRef::Deletion { .. } => {
                            file_entry(path_str, None, FileStatus::Deleted, true)
                        }
                        gix_diff::index::ChangeRef::Modification { .. } => {
                            file_entry(path_str, None, FileStatus::Modified, true)
                        }
                        gix_diff::index::ChangeRef::Rewrite { source_location, copy, .. } => {
                            file_entry(
                                path_str,
                                Some(source_location.to_string()),
                                if *copy {
                                    FileStatus::Copied
                                } else {
                                    FileStatus::Renamed
                                },
                                true,
                            )
                        }
                    };
                    result.staged.push(entry);
                }
                gix::status::Item::IndexWorktree(iw_item) => {
                    match iw_item {
                        gix::status::index_worktree::Item::Modification { rela_path, status, .. } => {
                            let path_str = rela_path.to_string();
                            match status {
                                gix_status::index_as_worktree::EntryStatus::Change(change) => {
                                    let file_status = match change {
                                        gix_status::index_as_worktree::Change::Removed => FileStatus::Deleted,
                                        gix_status::index_as_worktree::Change::Modification { .. } => FileStatus::Modified,
                                        gix_status::index_as_worktree::Change::Type { .. } => FileStatus::Modified,
                                        gix_status::index_as_worktree::Change::SubmoduleModification(_) => FileStatus::Modified,
                                    };
                                    result
                                        .unstaged
                                        .push(file_entry(path_str, None, file_status, false));
                                }
                                gix_status::index_as_worktree::EntryStatus::Conflict(_) => {
                                    result.conflicted.push(file_entry(
                                        path_str,
                                        None,
                                        FileStatus::Conflicted,
                                        false,
                                    ));
                                }
                                _ => {}
                            }
                        }
                        gix::status::index_worktree::Item::DirectoryContents { entry, .. } => {
                            if matches!(entry.status, gix_dir::entry::Status::Untracked) {
                                result.untracked.push(file_entry(
                                    entry.rela_path.to_string(),
                                    None,
                                    FileStatus::Untracked,
                                    false,
                                ));
                            }
                        }
                        gix::status::index_worktree::Item::Rewrite { source, dirwalk_entry, copy, .. } => {
                            result.unstaged.push(file_entry(
                                dirwalk_entry.rela_path.to_string(),
                                Some(source.rela_path().to_string()),
                                if *copy {
                                    FileStatus::Copied
                                } else {
                                    FileStatus::Renamed
                                },
                                false,
                            ));
                        }
                    }
                }
            }
        }

        self.attach_diff_stats(&mut result)?;
        Ok(result)
    }

    /// Spawns a `git` subprocess via `diff_numstat_vs_head`.
    pub fn attach_diff_stats(&self, status: &mut RepoStatus) -> GitResult<()> {
        let numstat = self.diff_numstat_vs_head().unwrap_or_default();
        let attach = |entry: &mut FileEntry| {
            if let Some(stat) = numstat.get(&entry.path) {
                entry.diff_stat = Some(*stat);
            } else if entry.status == FileStatus::Untracked {
                entry.diff_stat = untracked_line_count(self.path(), &entry.path);
            }
        };
        for entry in &mut status.staged {
            attach(entry);
        }
        for entry in &mut status.unstaged {
            attach(entry);
        }
        for entry in &mut status.untracked {
            attach(entry);
        }
        for entry in &mut status.conflicted {
            attach(entry);
        }
        Ok(())
    }
}
