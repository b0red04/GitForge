use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};

/// Entry point invoked by `main.rs`. Orchestrates the full update:
///
/// 1. Release file handles held by Explorer and other processes via the
///    Windows Restart Manager.
/// 2. Run a transactional sequence of file moves: live → `old\`,
///    staged (`install\`) → live.
/// 3. Clean up `old\`, `install\`, and `updates\`.
/// 4. Optionally launch the new `gitforge.exe`.
///
/// If any job fails after its retry budget, all previously successful jobs
/// are rolled back so the installation is left in a consistent state.
pub fn perform_update(app_dir: &Path, launch: bool) -> Result<()> {
    if let Err(e) = release_file_handles(app_dir) {
        // Not fatal — the retry loop in each Job may still succeed.
        eprintln!("Restart Manager failed (will continue anyway): {e}");
    }

    let mut last_successful_job: Option<usize> = None;
    for (i, job) in JOBS.iter().enumerate() {
        let start = Instant::now();
        loop {
            match (job.apply)(app_dir) {
                Ok(()) => {
                    last_successful_job = Some(i);
                    break;
                }
                Err(e) if start.elapsed() < RETRY_TIMEOUT => {
                    std::thread::sleep(RETRY_INTERVAL);
                    eprintln!("job {} retrying: {e}", job.name);
                }
                Err(e) => {
                    eprintln!("job {} failed permanently: {e}", job.name);
                    if let Some(last) = last_successful_job {
                        eprintln!("rolling back {} successful jobs...", last + 1);
                        for j in (0..=last).rev() {
                            if let Err(re) = (JOBS[j].rollback)(app_dir) {
                                eprintln!("rollback of job {} failed: {re}", JOBS[j].name);
                            }
                        }
                    }
                    return Err(e).with_context(|| format!("job '{}' failed", job.name));
                }
            }
        }
    }

    if launch {
        let exe = app_dir.join("gitforge.exe");
        let _ = std::process::Command::new(&exe).spawn();
    }

    println!("Update completed successfully");
    Ok(())
}

const RETRY_TIMEOUT: Duration = Duration::from_secs(2);
const RETRY_INTERVAL: Duration = Duration::from_millis(100);

type JobFn = Box<dyn Fn(&Path) -> Result<()> + Sync>;

struct Job {
    name: &'static str,
    apply: JobFn,
    rollback: JobFn,
}

impl Job {
    fn mkdir(name: &'static str, dir: &'static str) -> Self {
        Self {
            name,
            apply: Box::new(move |app_dir| {
                let path = app_dir.join(dir);
                std::fs::create_dir_all(&path).with_context(|| format!("mkdir {}", path.display()))
            }),
            rollback: Box::new(move |app_dir| {
                let path = app_dir.join(dir);
                let _ = std::fs::remove_dir(&path);
                Ok(())
            }),
        }
    }

    fn move_file(name: &'static str, from: &'static str, to: &'static str) -> Self {
        Self {
            name,
            apply: Box::new(move |app_dir| {
                let src = app_dir.join(from);
                let dst = app_dir.join(to);
                std::fs::rename(&src, &dst)
                    .with_context(|| format!("move {} -> {}", src.display(), dst.display()))
            }),
            rollback: Box::new(move |app_dir| {
                let src = app_dir.join(to);
                let dst = app_dir.join(from);
                let _ = std::fs::rename(&src, &dst);
                Ok(())
            }),
        }
    }

    fn rmdir_nofail(name: &'static str, dir: &'static str) -> Self {
        Self {
            name,
            apply: Box::new(move |app_dir| {
                let path = app_dir.join(dir);
                let _ = std::fs::remove_dir_all(&path);
                Ok(())
            }),
            rollback: Box::new(|_| Ok(())),
        }
    }
}

/// Transactional sequence of file operations that swap the old binary for
/// the new one staged in `install\`.
///
/// Layout before the swap:
/// ```text
/// <app_dir>/
///   gitforge.exe                    <- old (just exited)
///   gitforge-update-helper.exe      <- this process
///   install/
///     gitforge.exe                  <- new
///     gitforge-update-helper.exe    <- new
///   updates/
///     gitforge-update.zip           <- downloaded archive
/// ```
///
/// Layout after the swap:
/// ```text
/// <app_dir>/
///   gitforge.exe                    <- new
///   gitforge-update-helper.exe      <- new
/// ```
static JOBS: &[Job] = &[
    // 1. Create rollback directory.
    Job::mkdir("mkdir old", "old"),
    // 2. Move old live files to old\.
    Job::move_file(
        "move gitforge.exe -> old",
        "gitforge.exe",
        "old/gitforge.exe",
    ),
    Job::move_file(
        "move helper.exe -> old",
        "gitforge-update-helper.exe",
        "old/gitforge-update-helper.exe",
    ),
    // 3. Move new files from install\ to live.
    Job::move_file(
        "move install/gitforge.exe -> live",
        "install/gitforge.exe",
        "gitforge.exe",
    ),
    Job::move_file(
        "move install/helper.exe -> live",
        "install/gitforge-update-helper.exe",
        "gitforge-update-helper.exe",
    ),
    // 4. Clean up staging directories.
    Job::rmdir_nofail("rmdir updates", "updates"),
    Job::rmdir_nofail("rmdir install", "install"),
    Job::rmdir_nofail("rmdir old", "old"),
];

// ---------------------------------------------------------------------------
// Windows Restart Manager
// ---------------------------------------------------------------------------

/// Attempt to release file handles held by other processes (typically
/// Explorer's icon cache) on the main binary and helper.
///
/// Failures are non-fatal — the caller's retry loop provides a second line
/// of defence.
fn release_file_handles(app_dir: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::WIN32_ERROR;
    use windows::Win32::System::RestartManager::{
        CCH_RM_SESSION_KEY, RM_SHUTDOWN_TYPE, RmEndSession, RmRegisterResources, RmShutdown,
        RmStartSession,
    };
    use windows::core::PCWSTR;

    let files_to_release: Vec<PathBuf> = vec![
        app_dir.join("gitforge.exe"),
        app_dir.join("gitforge-update-helper.exe"),
    ];

    // Build null-terminated UTF-16 paths. These must outlive the PCWSTRs
    // derived from them below.
    let wide_paths: Vec<Vec<u16>> = files_to_release
        .iter()
        .map(|p| {
            let mut w: Vec<u16> = p.as_os_str().encode_wide().collect();
            w.push(0);
            w
        })
        .collect();
    let pcwstr_paths: Vec<PCWSTR> = wide_paths
        .iter()
        .map(|w| PCWSTR::from_raw(w.as_ptr()))
        .collect();

    let mut session: u32 = 0;
    let mut session_key = [0u16; (CCH_RM_SESSION_KEY as usize) + 1];

    // SAFETY: we pass valid pointers for the session handle and key buffer.
    // The session is ended by the scopeguard regardless of outcome.
    let err = unsafe {
        RmStartSession(
            &mut session,
            None,
            windows::core::PWSTR::from_raw(session_key.as_mut_ptr()),
        )
    };
    if err.is_err() {
        anyhow::bail!("RmStartSession failed: {err:?}");
    }

    let _guard = scopeguard::guard(session, |s| {
        let _ = unsafe { RmEndSession(s) };
    });

    let err = unsafe { RmRegisterResources(session, Some(&pcwstr_paths), None, None) };
    if err.is_err() {
        anyhow::bail!("RmRegisterResources failed: {err:?}");
    }

    // Ask processes to release their handles gracefully (flags = 0).
    let err = unsafe { RmShutdown(session, RM_SHUTDOWN_TYPE(0), None) };
    if err.is_err() {
        anyhow::bail!("RmShutdown failed: {err:?}");
    }

    let _ = WIN32_ERROR::default(); // suppress unused-import warning if any
    Ok(())
}
