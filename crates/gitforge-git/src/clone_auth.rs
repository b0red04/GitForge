use std::process::Command;

use crate::error::{GitError, GitResult};

// Git invokes helpers through its shell. Secrets are read from the child
// environment, never interpolated into shell code or command-line arguments.
const HELPER: &str = r#"!f() {
    test "$1" = get || exit 0
    protocol= host= path=
    while IFS='=' read -r key value; do
        case "$key" in
            protocol) protocol=$value ;;
            host) host=$value ;;
            path) path=$value ;;
        esac
    done
    if test "$protocol://$host/$path" = "$GITFORGE_CLONE_URL"; then
        printf 'username=%s\npassword=%s\n' "$GITFORGE_CLONE_USERNAME" "$GITFORGE_CLONE_TOKEN"
    else
        printf 'quit=true\n'
    fi
}; f"#;

pub(crate) fn configure(
    command: &mut Command,
    url: &str,
    username: &str,
    token: &str,
) -> GitResult<()> {
    let valid_url = url
        .strip_prefix("https://")
        .and_then(|rest| rest.split_once('/'))
        .is_some_and(|(host, path)| {
            !host.is_empty()
                && !path.is_empty()
                && !host.contains('@')
                && !url
                    .chars()
                    .any(|c| c.is_whitespace() || matches!(c, '\0' | '?' | '#' | '\\'))
        });
    if !valid_url
        || [username, token]
            .iter()
            .any(|value| value.is_empty() || value.contains(['\n', '\r', '\0']))
    {
        return Err(GitError::OperationFailed(
            "Invalid HTTPS clone credentials or URL".into(),
        ));
    }

    command
        .args([
            "-c",
            "credential.helper=",
            "-c",
            &format!("credential.helper={HELPER}"),
        ])
        .args([
            "-c",
            "credential.useHttpPath=true",
            "-c",
            "http.followRedirects=false",
        ])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GITFORGE_CLONE_URL", url)
        .env("GITFORGE_CLONE_USERNAME", username)
        .env("GITFORGE_CLONE_TOKEN", token);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::process::Stdio;

    fn fill(request: &str) -> std::process::Output {
        let mut command = Command::new("git");
        command
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null");
        // An existing helper must not override or persist the selected account.
        command.args([
            "-c",
            "credential.helper=!echo username=wrong; echo password=wrong",
        ]);
        configure(
            &mut command,
            "https://github.com/owner/private.git",
            "selected",
            "dummy-';$token",
        )
        .unwrap();
        assert!(
            !command
                .get_args()
                .any(|arg| arg.to_string_lossy().contains("dummy-"))
        );
        let mut child = command
            .args(["credential", "fill"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(request.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    #[test]
    fn selected_account_supplies_private_clone_credentials() {
        let output = fill("protocol=https\nhost=github.com\npath=owner/private.git\n\n");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("username=selected\n"));
        assert!(stdout.contains("password=dummy-';$token\n"));
    }

    #[test]
    fn credentials_are_not_returned_for_other_repositories_or_hosts() {
        for request in [
            "protocol=https\nhost=other.example\npath=owner/private.git\n\n",
            "protocol=https\nhost=github.com\npath=other/private.git\n\n",
            "protocol=http\nhost=github.com\npath=owner/private.git\n\n",
        ] {
            let output = fill(request);
            assert!(!output.status.success());
            assert!(!String::from_utf8_lossy(&output.stdout).contains("dummy-"));
            assert!(!String::from_utf8_lossy(&output.stderr).contains("dummy-"));
        }
    }

    #[test]
    fn rejects_unsafe_urls_and_credential_protocol_injection() {
        for url in [
            "http://github.com/o/r",
            "https://user@github.com/o/r",
            "https://github.com/o/r?x",
            "https://github.com/o/r\n",
        ] {
            assert!(configure(&mut Command::new("git"), url, "user", "token").is_err());
        }
        assert!(
            configure(
                &mut Command::new("git"),
                "https://github.com/o/r",
                "user",
                "token\npassword=other"
            )
            .is_err()
        );
    }
}
