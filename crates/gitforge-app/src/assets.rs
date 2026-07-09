use gpui::{AssetSource, SharedString};
use std::borrow::Cow;
use std::collections::HashMap;

pub struct EmbeddedAssets {
    files: HashMap<&'static str, &'static [u8]>,
}

impl Default for EmbeddedAssets {
    fn default() -> Self {
        Self::new()
    }
}

impl EmbeddedAssets {
    pub fn new() -> Self {
        let mut files = HashMap::new();

        macro_rules! embed {
            ($path:literal) => {
                files.insert(
                    $path,
                    include_bytes!(concat!("../../../assets/", $path)).as_slice(),
                );
            };
        }

        embed!("icons/generic_close.svg");
        embed!("icons/generic_minimize.svg");
        embed!("icons/generic_maximize.svg");
        embed!("icons/generic_restore.svg");
        embed!("icons/git-commit.svg");
        embed!("icons/git-branch.svg");
        embed!("icons/git-merge.svg");
        embed!("icons/git_merge_conflict.svg");
        embed!("icons/git-pull-request.svg");
        embed!("icons/github.svg");
        embed!("icons/gitlab.svg");
        embed!("icons/tag.svg");
        embed!("icons/file.svg");
        embed!("icons/folder.svg");
        embed!("icons/search.svg");
        embed!("icons/settings.svg");
        embed!("icons/plus.svg");
        embed!("icons/check.svg");
        embed!("icons/x.svg");
        embed!("icons/chevron-down.svg");
        embed!("icons/chevron-right.svg");
        embed!("icons/arrow-down.svg");
        embed!("icons/arrow-up.svg");
        embed!("icons/refresh.svg");
        embed!("icons/loader.svg");
        embed!("icons/cloud.svg");
        embed!("icons/terminal.svg");
        embed!("icons/globe.svg");
        embed!("icons/laptop.svg");

        Self { files }
    }
}

impl AssetSource for EmbeddedAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(self.files.get(path).map(|bytes| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(self
            .files
            .keys()
            .filter(|k| k.starts_with(path))
            .map(|k| SharedString::from(*k))
            .collect())
    }
}
