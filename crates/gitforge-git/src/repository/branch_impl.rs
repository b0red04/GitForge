use crate::error::GitResult;
use crate::repository::Repository;
use std::collections::HashSet;

impl Repository {
    /// Spawns a `git` subprocess.
    pub fn create_branch(&self, name: &str, start_point: Option<&str>) -> GitResult<()> {
        let mut args = vec!["branch", name];
        if let Some(sp) = start_point {
            args.push(sp);
        }
        self.run_git(&args)?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn delete_branch(&self, name: &str, force: bool) -> GitResult<()> {
        let flag = if force { "-D" } else { "-d" };
        self.run_git(&["branch", flag, name])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn rename_branch(&self, old: &str, new: &str) -> GitResult<()> {
        self.run_git(&["branch", "-m", old, new])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn checkout_branch(&self, name: &str) -> GitResult<()> {
        self.run_git(&["checkout", name])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn checkout_commit(&self, sha: &str) -> GitResult<()> {
        self.run_git(&["checkout", sha])?;
        Ok(())
    }

    pub fn main_branch_name(&self) -> GitResult<Option<String>> {
        for branch in ["main", "master"] {
            if self
                .run_git(&["rev-parse", "--verify", &format!("refs/heads/{branch}")])
                .is_ok()
            {
                return Ok(Some(branch.to_string()));
            }
        }

        for branch in ["origin/main", "origin/master"] {
            if self
                .run_git(&["rev-parse", "--verify", &format!("refs/remotes/{branch}")])
                .is_ok()
            {
                return Ok(Some(branch.to_string()));
            }
        }

        Ok(None)
    }

    pub fn local_branches_conflicting_with_main(
        &self,
        branches: &[String],
    ) -> GitResult<HashSet<String>> {
        let Some(base) = self.main_branch_name()? else {
            return Ok(HashSet::new());
        };

        let mut conflicting = HashSet::new();
        for branch in branches {
            if branch == &base || format!("origin/{branch}") == base {
                continue;
            }

            if self.branch_conflicts_with_base(&base, branch)? {
                conflicting.insert(branch.clone());
            }
        }

        Ok(conflicting)
    }

    pub fn branch_conflicts_with_base(&self, base: &str, branch: &str) -> GitResult<bool> {
        match self.run_git(&["merge-tree", "--write-tree", base, branch]) {
            Ok(_) => Ok(false),
            Err(err) => {
                let message = err.to_string();
                if message.contains("usage: git merge-tree")
                    || message.contains("unknown option")
                    || message.contains("not a git command")
                {
                    tracing::warn!(
                        "git merge-tree is unavailable for conflict detection: {}",
                        message
                    );
                    return Ok(false);
                }

                tracing::warn!(
                    "Unexpected git merge-tree error, assuming no conflict: {}",
                    message
                );
                Ok(false)
            }
        }
    }

    /// Spawns a `git` subprocess.
    pub fn create_tag(
        &self,
        name: &str,
        message: Option<&str>,
        target: Option<&str>,
    ) -> GitResult<()> {
        let mut args = vec!["tag"];
        if let Some(msg) = message {
            args.extend(["-a", name, "-m", msg]);
        } else {
            args.push(name);
        }
        if let Some(t) = target {
            args.push(t);
        }
        self.run_git(&args)?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn delete_tag(&self, name: &str) -> GitResult<()> {
        self.run_git(&["tag", "-d", name])?;
        Ok(())
    }
}
