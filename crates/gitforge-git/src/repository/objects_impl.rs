use crate::error::{GitError, GitResult};
use crate::repository::Repository;
use std::path::Path;

impl Repository {
    pub fn file_at_commit(&self, commit_id: &str, file_path: &Path) -> GitResult<Option<Vec<u8>>> {
        let tree = self.find_commit_tree(commit_id)?;

        let Some(entry) = tree
            .lookup_entry_by_path(file_path)
            .map_err(|e| GitError::OperationFailed(e.to_string()))?
        else {
            return Ok(None);
        };

        let blob = entry
            .object()
            .map_err(|e| GitError::OperationFailed(e.to_string()))?
            .try_into_blob()
            .map_err(|e| GitError::OperationFailed(format!("Not a blob: {:?}", e)))?;

        Ok(Some(blob.data.clone()))
    }

    pub fn list_files_at_commit(&self, commit_id: &str) -> GitResult<Vec<String>> {
        let tree = self.find_commit_tree(commit_id)?;

        let entries = tree
            .traverse()
            .breadthfirst
            .files()
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;

        Ok(entries.iter().map(|e| e.filepath.to_string()).collect())
    }

    pub fn blob_content(&self, object_id: &str) -> GitResult<Option<String>> {
        let id = self.parse_object_id(object_id, "object ID")?;

        let obj = self
            .repo
            .find_object(id)
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;

        match obj.try_into_blob() {
            Ok(blob) => Ok(Some(String::from_utf8_lossy(&blob.data).to_string())),
            Err(_) => Ok(None),
        }
    }
}
