use crate::error::GitResult;
use crate::repository::Repository;
use std::path::Path;

impl Repository {
    /// Spawns a `git` subprocess.
    pub fn submodule_status(&self) -> GitResult<String> {
        let output = self.run_git(&["submodule", "status"])?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Spawns a `git` subprocess.
    pub fn submodule_init(&self, path: Option<&Path>) -> GitResult<()> {
        let mut args = vec!["submodule", "init"];
        if let Some(p) = path {
            args.push("--");
            args.push(p.to_str().unwrap_or(""));
        }
        self.run_git(&args)?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn submodule_update(&self, path: Option<&Path>, init: bool, recursive: bool) -> GitResult<()> {
        let mut args = vec!["submodule", "update"];
        if init {
            args.push("--init");
        }
        if recursive {
            args.push("--recursive");
        }
        if let Some(p) = path {
            args.push("--");
            args.push(p.to_str().unwrap_or(""));
        }
        self.run_git(&args)?;
        Ok(())
    }
}
