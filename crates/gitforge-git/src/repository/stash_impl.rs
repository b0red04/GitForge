use crate::error::GitResult;
use crate::repository::Repository;

impl Repository {
    /// Spawns a `git` subprocess.
    pub fn stash_push(&self, message: Option<&str>) -> GitResult<()> {
        let mut args = vec!["stash", "push"];
        if let Some(msg) = message {
            args.extend(["-m", msg]);
        }
        self.run_git(&args)?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn stash_pop(&self) -> GitResult<()> {
        self.run_git(&["stash", "pop"])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn stash_apply(&self) -> GitResult<()> {
        self.run_git(&["stash", "apply"])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn stash_drop(&self) -> GitResult<()> {
        self.run_git(&["stash", "drop"])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn stash_list(&self) -> GitResult<String> {
        let output = self.run_git(&["stash", "list"])?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
