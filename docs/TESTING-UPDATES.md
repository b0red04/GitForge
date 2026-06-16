# Testing auto-updates

Release builds installed via the install script check GitHub hourly for newer releases and apply them automatically. Auto-updates are disabled in debug builds (`cargo run`); use a release install or the workflows below.

## End-to-end against GitHub

If you have an older release installed and a newer one is published on GitHub, launch the installed app and either wait for the hourly check or trigger one manually. The app should download the new tarball, verify the checksum, install, and prompt to restart.

## Restart button (no release required)

The restart affordance can be tested without publishing a new version:

```bash
./scripts/test-update-restart.sh
```

This script:

1. Builds a release binary (`cargo build -p gitforge-app --release`)
2. Sets `GITFORGE_DEV_SIMULATE_UPDATE_READY=1` so the title bar shows **Restart to update** without contacting GitHub
3. Sets `GITFORGE_DEV_RESTART_PATH` to a wrapper that logs to `/tmp/gitforge-restart-test.log` and then execs the real binary

Click **Restart to update**, then confirm the log contains a `RELAUNCHED` line:

```bash
cat /tmp/gitforge-restart-test.log
```

For more detail while testing:

```bash
RUST_LOG=info ./scripts/test-update-restart.sh
```

Look for `restarting to apply update` and GPUI's `Restarting process, using app path:` in the output.

### Dev environment variables

| Variable                               | Purpose                                                                  |
| -------------------------------------- | ------------------------------------------------------------------------ |
| `GITFORGE_DEV_SIMULATE_UPDATE_READY=1` | Show the pending-update UI on startup (skips download/install)           |
| `GITFORGE_DEV_RESTART_PATH`            | Binary path GPUI uses when restarting (defaults to the running app path) |

## Automated regression tests

```bash
cargo test -p gitforge-update restart_to_apply_update_reaches_restart
cargo test -p gitforge-app --test update_restart_click
```

- `restart_to_apply_update_reaches_restart` — verifies `restart_to_apply_update` closes windows and calls `cx.restart()`
- `update_restart_click` — simulates a title-bar click inside a drag region (the failure mode where the button appeared to do nothing)

## Full update pipeline locally (optional)

To exercise download and install without publishing to GitHub, build a tarball locally and host it yourself:

```bash
cargo build -p gitforge-app --release
./scripts/build-update-tarball.sh 99.99.99
```

Serve `GitForge-99.99.99-<arch>.tar.gz` and its `.sha256` file from a local HTTP server and point the updater at that release metadata. This is heavier than the restart-only workflow above; use it when you need to validate rsync install paths and checksum verification.
