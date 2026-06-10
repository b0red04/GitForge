use gpui::SharedString;
use parking_lot::RwLock;
use std::collections::HashMap;

pub struct IconBank {
    icons: RwLock<HashMap<String, SharedString>>,
}

impl IconBank {
    pub fn new() -> Self {
        let mut icons = HashMap::new();

        icons.insert(
            "git-commit".into(),
            include_str!("../../../assets/icons/git-commit.svg").into(),
        );
        icons.insert(
            "git-branch".into(),
            include_str!("../../../assets/icons/git-branch.svg").into(),
        );
        icons.insert(
            "git-merge".into(),
            include_str!("../../../assets/icons/git-merge.svg").into(),
        );
        icons.insert(
            "git-pull-request".into(),
            include_str!("../../../assets/icons/git-pull-request.svg").into(),
        );
        icons.insert(
            "tag".into(),
            include_str!("../../../assets/icons/tag.svg").into(),
        );
        icons.insert(
            "file".into(),
            include_str!("../../../assets/icons/file.svg").into(),
        );
        icons.insert(
            "folder".into(),
            include_str!("../../../assets/icons/folder.svg").into(),
        );
        icons.insert(
            "search".into(),
            include_str!("../../../assets/icons/search.svg").into(),
        );
        icons.insert(
            "settings".into(),
            include_str!("../../../assets/icons/settings.svg").into(),
        );
        icons.insert(
            "plus".into(),
            include_str!("../../../assets/icons/plus.svg").into(),
        );
        icons.insert(
            "check".into(),
            include_str!("../../../assets/icons/check.svg").into(),
        );
        icons.insert(
            "x".into(),
            include_str!("../../../assets/icons/x.svg").into(),
        );
        icons.insert(
            "chevron-down".into(),
            include_str!("../../../assets/icons/chevron-down.svg").into(),
        );
        icons.insert(
            "chevron-right".into(),
            include_str!("../../../assets/icons/chevron-right.svg").into(),
        );
        icons.insert(
            "refresh".into(),
            include_str!("../../../assets/icons/refresh.svg").into(),
        );
        icons.insert(
            "cloud".into(),
            include_str!("../../../assets/icons/cloud.svg").into(),
        );
        icons.insert(
            "terminal".into(),
            include_str!("../../../assets/icons/terminal.svg").into(),
        );

        Self {
            icons: RwLock::new(icons),
        }
    }

    pub fn get(&self, name: &str) -> Option<SharedString> {
        self.icons.read().get(name).cloned()
    }

    pub fn register(&self, name: &str, svg: SharedString) {
        self.icons.write().insert(name.into(), svg);
    }
}

impl Default for IconBank {
    fn default() -> Self {
        Self::new()
    }
}
