pub fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or(s).trim().to_string()
}

/// Redacts credential userinfo from URLs embedded in a string. Replaces the
/// `user:password@` or `token@` portion of any `scheme://...@host` URL with
/// `***@` so tokens/passwords don't leak into toast messages.
pub fn redact_credentials(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut remaining = s;

    while let Some(scheme_end) = remaining.find("://") {
        result.push_str(&remaining[..scheme_end + 3]);
        let after_scheme = &remaining[scheme_end + 3..];

        let host_end = after_scheme
            .find(['/', ' ', '\'', '"'])
            .unwrap_or(after_scheme.len());
        let authority = &after_scheme[..host_end];

        if let Some(at_pos) = authority.rfind('@') {
            result.push_str("***@");
            result.push_str(&authority[at_pos + 1..]);
        } else {
            result.push_str(authority);
        }
        remaining = &after_scheme[host_end..];
    }
    result.push_str(remaining);
    result
}

pub fn redact_for_display(s: &str) -> String {
    redact_credentials(&first_line(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_credentials_strips_userinfo_from_urls() {
        assert_eq!(
            redact_credentials("https://user:token@github.com/repo.git"),
            "https://***@github.com/repo.git"
        );
        assert_eq!(
            redact_credentials("fatal: failed for 'https://x:yz@host.com/path'"),
            "fatal: failed for 'https://***@host.com/path'"
        );
        assert_eq!(
            redact_credentials("https://github.com/owner/repo.git"),
            "https://github.com/owner/repo.git"
        );
        assert_eq!(redact_credentials("nothing to commit"), "nothing to commit");
    }
}
