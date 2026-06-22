//! Shared helpers for hosting-provider characterization tests.
//!
//! Tests point providers at an `httpmock::MockServer` via the `*Provider::with_url`
//! constructors. Because trait methods take `&HostingAccount` (which reads its
//! token from the secrets backend), tests store throwaway tokens.
//!
//! ## Isolation from the user's real config
//!
//! `secrets::store_token` reads/writes `hosting_tokens.json` under the config
//! dir. To prevent parallel test binaries from clobbering the user's real
//! tokens, [`ensure_test_tokens`] redirects the hosting crate at a per-process
//! temp dir via `GITFORGE_CONFIG_DIR` before any token write. Each process
//! then writes the same set of test keys into its own file, so there are no
//! cross-process races and the user's `~/.config/gitforge/hosting_tokens.json`
//! is never touched.

use gitforge_hosting::HostingAccount;
use std::sync::OnceLock;

/// The token file path — mirrors `secrets::tokens_file()` (private), honoring
/// the same `GITFORGE_CONFIG_DIR` override.
fn token_file_path() -> std::path::PathBuf {
    let base = std::env::var("GITFORGE_CONFIG_DIR").map(std::path::PathBuf::from);
    base.unwrap_or_else(|_| {
        dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("gitforge")
    })
    .join("hosting_tokens.json")
}

/// Write all test-known tokens to the file in a single pass, merging with any
/// existing entries.  Idempotent: calling it again is a no-op.  Called once per
/// process via [`ensure_test_tokens`].
fn seed_token_file() {
    let path = token_file_path();

    // Load existing tokens (preserve the user's real tokens).
    let mut tokens: serde_json::Map<String, serde_json::Value> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .and_then(|v: serde_json::Value| v.as_object().cloned())
        .unwrap_or_default();

    // Add test tokens.
    for (k, v) in ALL_TEST_TOKENS {
        tokens.insert((*k).to_string(), serde_json::json!(*v));
    }

    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let _ = std::fs::write(&path, serde_json::to_string(&tokens).unwrap());
}

/// All token keys used by any test in any binary.  Keeping them in one place
/// ensures a single file write covers everything.
const ALL_TEST_TOKENS: &[(&str, &str)] = &[
    ("test:hosting", "test-token"),
    // Keys generated internally by `authenticate` — pre-seeded so the verify
    // step inside `store_token` succeeds even under cross-process races.
    ("github:octocat", "ghp_test"),
    ("codeberg:alice", "cb_test"),
    ("gitlab:bob", "gl_test"),
];

/// Seed the token file exactly once per process.  Safe to call from any test.
pub fn ensure_test_tokens() {
    static DONE: OnceLock<()> = OnceLock::new();
    DONE.get_or_init(|| {
        // Redirect the hosting crate's `secrets::config_dir()` at a
        // per-process temp dir so this test binary never reads or writes the
        // user's real `~/.config/gitforge/hosting_tokens.json`. Each parallel
        // test binary gets its own dir (keyed by PID), eliminating the
        // cross-process read-modify-write race that previously lost user
        // tokens.
        let temp_dir = std::env::temp_dir().join(format!("gitforge-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        // SAFETY: this runs once inside `OnceLock::get_or_init`, before any
        // test body runs. Test binaries are single-process; the only threads
        // alive at this point are the test harness's, which haven't started
        // reading `GITFORGE_CONFIG_DIR` yet (the first such read is inside
        // `secrets::config_dir()` triggered by a test body, below).
        unsafe {
            std::env::set_var("GITFORGE_CONFIG_DIR", &temp_dir);
        }
        seed_token_file();
    });
}

/// Returns a `HostingAccount` backed by the shared test token (`"test:hosting"`).
///
/// All non-`authenticate` tests use this.  The token is intentionally **not**
/// cleaned up — leaving it in the file avoids a delete-while-other-tests-read
/// race.
pub fn test_account(provider: &str, username: &str) -> HostingAccount {
    ensure_test_tokens();
    HostingAccount {
        provider: provider.to_string(),
        username: username.to_string(),
        display_name: username.to_string(),
        avatar_url: None,
        token_key: "test:hosting".to_string(),
        created_at: None,
    }
}
