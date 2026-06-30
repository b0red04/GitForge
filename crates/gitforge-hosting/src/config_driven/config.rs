//! Per-provider configuration rows and shared family definitions.

pub(crate) enum AuthStyle {
    Authorization { scheme: &'static str },
    HeaderToken { name: &'static str },
}

pub(crate) enum SearchStyle {
    Dedicated {
        path: &'static str,
        envelope: &'static str,
        query_param: &'static str,
    },
    ProjectFilter {
        query_param: &'static str,
        page_size: usize,
        query_extra: &'static str,
    },
}

pub(crate) enum ResourcePaths {
    ReposSegmented,
    ProjectsEncoded,
}

pub(crate) struct EndpointSuffixes {
    pub fork: &'static str,
    pub pr: &'static str,
    pub branches: &'static str,
}

pub(crate) enum PullRequestStrategy {
    HeadColonOnTarget,
    TargetProjectIdOnSource,
}

pub(crate) enum ForkDetection {
    Bool(&'static str),
    ObjectExists(&'static str),
}

pub(crate) enum PrivateDetection {
    Bool(&'static str),
    VisibilityPrivate(&'static str),
}

pub(crate) enum PrHeadBranchKey {
    Nested(&'static str, &'static str),
    Flat(&'static str),
}

pub(crate) struct RepoJsonKeys {
    pub full_name: &'static str,
    pub clone_url: &'static str,
    pub ssh_url: &'static str,
    pub html_url: &'static str,
    pub updated_at: &'static str,
    pub stars: &'static str,
    pub fork: ForkDetection,
    pub private: PrivateDetection,
}

pub(crate) struct PrJsonKeys {
    pub number: &'static str,
    pub html_url: &'static str,
    pub state_default: &'static str,
    pub head_branch: PrHeadBranchKey,
}

/// Bundles path shape, endpoint suffixes, PR strategy, and JSON dialect together
/// so invalid cross-product combinations are unrepresentable.
pub(crate) struct ProviderFamily {
    pub resource_paths: ResourcePaths,
    pub suffixes: EndpointSuffixes,
    pub pr_strategy: PullRequestStrategy,
    pub repo_keys: RepoJsonKeys,
    pub pr_keys: PrJsonKeys,
}

pub(crate) struct ProviderConfig {
    pub id: &'static str,
    pub display_name: &'static str,
    pub default_base_url: &'static str,
    pub default_web_url: &'static str,
    pub api_suffix: &'static str,
    pub auth: AuthStyle,
    pub accept: Option<&'static str>,
    pub page_param: &'static str,
    pub page_size: usize,
    pub sort_repos: bool,
    pub username_key: &'static str,
    pub display_name_key: &'static str,
    pub list_repos_path: &'static str,
    pub list_repos_query_extra: &'static str,
    pub search_style: SearchStyle,
    pub list_repos_envelope: &'static str,
    pub pr_open_state: &'static str,
    pub encode_web_repo_path: bool,
    pub family: ProviderFamily,
}

// --- Shared Gitea-segmented family (GitHub, Codeberg) ---

const GITEA_SUFFIXES: EndpointSuffixes = EndpointSuffixes {
    fork: "forks",
    pr: "pulls",
    branches: "branches",
};

const GITEA_PR_KEYS: PrJsonKeys = PrJsonKeys {
    number: "number",
    html_url: "html_url",
    state_default: "open",
    head_branch: PrHeadBranchKey::Nested("head", "ref"),
};

const fn gitea_repo_keys(stars: &'static str) -> RepoJsonKeys {
    RepoJsonKeys {
        full_name: "full_name",
        clone_url: "clone_url",
        ssh_url: "ssh_url",
        html_url: "html_url",
        updated_at: "updated_at",
        stars,
        fork: ForkDetection::Bool("fork"),
        private: PrivateDetection::Bool("private"),
    }
}

const fn gitea_family(stars_key: &'static str) -> ProviderFamily {
    ProviderFamily {
        resource_paths: ResourcePaths::ReposSegmented,
        suffixes: GITEA_SUFFIXES,
        pr_strategy: PullRequestStrategy::HeadColonOnTarget,
        repo_keys: gitea_repo_keys(stars_key),
        pr_keys: GITEA_PR_KEYS,
    }
}

const GITLAB_FAMILY: ProviderFamily = ProviderFamily {
    resource_paths: ResourcePaths::ProjectsEncoded,
    suffixes: EndpointSuffixes {
        fork: "fork",
        pr: "merge_requests",
        branches: "repository/branches",
    },
    pr_strategy: PullRequestStrategy::TargetProjectIdOnSource,
    repo_keys: RepoJsonKeys {
        full_name: "path_with_namespace",
        clone_url: "http_url_to_repo",
        ssh_url: "ssh_url_to_repo",
        html_url: "web_url",
        updated_at: "last_activity_at",
        stars: "star_count",
        fork: ForkDetection::ObjectExists("forked_from_project"),
        private: PrivateDetection::VisibilityPrivate("visibility"),
    },
    pr_keys: PrJsonKeys {
        number: "iid",
        html_url: "web_url",
        state_default: "opened",
        head_branch: PrHeadBranchKey::Flat("source_branch"),
    },
};

pub(crate) const GITHUB: ProviderConfig = ProviderConfig {
    id: "github",
    display_name: "GitHub",
    default_base_url: "https://api.github.com",
    default_web_url: "https://github.com",
    api_suffix: "",
    auth: AuthStyle::Authorization { scheme: "Bearer" },
    accept: Some("application/vnd.github+json"),
    page_param: "per_page",
    page_size: 100,
    sort_repos: true,
    username_key: "login",
    display_name_key: "name",
    list_repos_path: "/user/repos",
    list_repos_query_extra: "sort=updated",
    search_style: SearchStyle::Dedicated {
        path: "/search/repositories",
        envelope: "items",
        query_param: "q",
    },
    list_repos_envelope: "",
    pr_open_state: "open",
    encode_web_repo_path: false,
    family: gitea_family("stargazers_count"),
};

pub(crate) const CODEBERG: ProviderConfig = ProviderConfig {
    id: "codeberg",
    display_name: "Codeberg",
    default_base_url: "https://codeberg.org/api/v1",
    default_web_url: "https://codeberg.org",
    api_suffix: "/api/v1",
    auth: AuthStyle::Authorization { scheme: "token" },
    accept: None,
    page_param: "limit",
    page_size: 50,
    sort_repos: false,
    username_key: "login",
    display_name_key: "full_name",
    list_repos_path: "/user/repos",
    list_repos_query_extra: "sort=updated",
    search_style: SearchStyle::Dedicated {
        path: "/repos/search",
        envelope: "data",
        query_param: "q",
    },
    list_repos_envelope: "",
    pr_open_state: "open",
    encode_web_repo_path: false,
    family: gitea_family("stars_count"),
};

pub(crate) const GITLAB: ProviderConfig = ProviderConfig {
    id: "gitlab",
    display_name: "GitLab",
    default_base_url: "https://gitlab.com/api/v4",
    default_web_url: "https://gitlab.com",
    api_suffix: "/api/v4",
    auth: AuthStyle::HeaderToken {
        name: "PRIVATE-TOKEN",
    },
    accept: None,
    page_param: "per_page",
    page_size: 100,
    sort_repos: false,
    username_key: "username",
    display_name_key: "name",
    list_repos_path: "/projects",
    list_repos_query_extra: "membership=true&order_by=updated_at",
    search_style: SearchStyle::ProjectFilter {
        query_param: "search",
        page_size: 30,
        query_extra: "order_by=updated_at",
    },
    list_repos_envelope: "",
    pr_open_state: "opened",
    encode_web_repo_path: true,
    family: GITLAB_FAMILY,
};

pub(crate) fn config_for_id(id: &str) -> Option<&'static ProviderConfig> {
    match id {
        "github" => Some(&GITHUB),
        "codeberg" => Some(&CODEBERG),
        "gitlab" => Some(&GITLAB),
        _ => None,
    }
}

pub(crate) enum EndpointAction {
    Fork,
    PullRequest,
    Branch,
}

pub(crate) fn resource_prefix(cfg: &ProviderConfig, owner: &str, repo: &str) -> String {
    match cfg.family.resource_paths {
        ResourcePaths::ReposSegmented => format!("/repos/{owner}/{repo}"),
        ResourcePaths::ProjectsEncoded => format!(
            "/projects/{}",
            crate::http::url_encode_path(&format!("{owner}/{repo}"))
        ),
    }
}

pub(crate) fn collection_url(
    base_url: &str,
    cfg: &ProviderConfig,
    owner: &str,
    repo: &str,
    action: EndpointAction,
) -> String {
    let suffix = match action {
        EndpointAction::Fork => cfg.family.suffixes.fork,
        EndpointAction::PullRequest => cfg.family.suffixes.pr,
        EndpointAction::Branch => cfg.family.suffixes.branches,
    };
    format!("{}{}/{}", base_url, resource_prefix(cfg, owner, repo), suffix)
}
