use crate::error::{GitError, GitResult};
use crate::repository::Repository;
use crate::status::{FileEntry, FileStatus, RepoStatus};

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
                        gix_diff::index::ChangeRef::Addition { .. } => FileEntry {
                            path: path_str,
                            old_path: None,
                            status: FileStatus::Added,
                            staged: true,
                        },
                        gix_diff::index::ChangeRef::Deletion { .. } => FileEntry {
                            path: path_str,
                            old_path: None,
                            status: FileStatus::Deleted,
                            staged: true,
                        },
                        gix_diff::index::ChangeRef::Modification { .. } => FileEntry {
                            path: path_str,
                            old_path: None,
                            status: FileStatus::Modified,
                            staged: true,
                        },
                        gix_diff::index::ChangeRef::Rewrite { source_location, copy, .. } => FileEntry {
                            path: path_str,
                            old_path: Some(source_location.to_string()),
                            status: if *copy { FileStatus::Copied } else { FileStatus::Renamed },
                            staged: true,
                        },
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
                                    result.unstaged.push(FileEntry {
                                        path: path_str,
                                        old_path: None,
                                        status: file_status,
                                        staged: false,
                                    });
                                }
                                gix_status::index_as_worktree::EntryStatus::Conflict(_) => {
                                    result.conflicted.push(FileEntry {
                                        path: path_str,
                                        old_path: None,
                                        status: FileStatus::Conflicted,
                                        staged: false,
                                    });
                                }
                                _ => {}
                            }
                        }
                        gix::status::index_worktree::Item::DirectoryContents { entry, .. } => {
                            if matches!(entry.status, gix_dir::entry::Status::Untracked) {
                                result.untracked.push(FileEntry {
                                    path: entry.rela_path.to_string(),
                                    old_path: None,
                                    status: FileStatus::Untracked,
                                    staged: false,
                                });
                            }
                        }
                        gix::status::index_worktree::Item::Rewrite { source, dirwalk_entry, copy, .. } => {
                            result.unstaged.push(FileEntry {
                                path: dirwalk_entry.rela_path.to_string(),
                                old_path: Some(source.rela_path().to_string()),
                                status: if *copy { FileStatus::Copied } else { FileStatus::Renamed },
                                staged: false,
                            });
                        }
                    }
                }
            }
        }

        Ok(result)
    }
}
