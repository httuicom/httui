//! Detect which forge a `git remote` URL points at.
//!
//! Used to compose forge-specific URLs (`<origin>/blob/<sha>/<path>`
//! for GitHub, `<origin>/-/blob/...` for GitLab) and compare/PR URLs.
//! Pure parsing; no network.
//!
//! Accepts both SSH (`git@github.com:owner/repo.git`) and HTTPS
//! (`https://github.com/owner/repo.git`) shapes, with or without
//! the trailing `.git`. Anything we can't classify falls into
//! `Other(host)` so the consumer can still surface "open <origin>".
//!
//! Self-hosted GitLab is detected by the path segment heuristic:
//! a URL whose host is unknown but whose path starts with at least
//! two segments (`/owner/repo`) is treated as a self-hosted forge.
//! Without out-of-band configuration we can't reliably know if a
//! given private host is GitLab vs Gitea — the consumer falls back
//! to "Manual: open <origin>" in that case.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum RemoteHost {
    Github,
    Gitlab,
    /// GitLab on a custom host (e.g. `gitlab.example.com`). Composes
    /// the same URL shape as `Gitlab` but the consumer keeps the
    /// host so the rendered URL points at the right server.
    GitlabSelfHosted(String),
    Bitbucket,
    Gitea,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParsedRemote {
    pub host: RemoteHost,
    /// Bare host string for display / GitlabSelfHosted construction.
    pub host_str: String,
    pub owner: String,
    pub repo: String,
    /// The original URL, untouched, so the consumer can fall back to
    /// "open <origin>" when host detection fails.
    pub original: String,
}

/// Parse a git remote URL into `(host, owner, repo)`. Returns `None`
/// when the URL doesn't carry an `owner/repo` path — anything else
/// (including unknown hosts) is recorded as `RemoteHost::Other`.
pub fn parse_remote_url(url: &str) -> Option<ParsedRemote> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (host, path) = if let Some(stripped) = trimmed.strip_prefix("git@") {
        // SSH: git@host:owner/repo[.git]
        let (h, p) = stripped.split_once(':')?;
        (h.to_string(), p.to_string())
    } else if let Some(stripped) = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .or_else(|| trimmed.strip_prefix("ssh://"))
        .or_else(|| trimmed.strip_prefix("git://"))
    {
        // <scheme>://[user@]host[:port]/owner/repo[.git]
        let (h, p) = stripped.split_once('/')?;
        let h = h.split('@').next_back().unwrap_or(h);
        let h = h.split(':').next().unwrap_or(h);
        (h.to_string(), p.to_string())
    } else {
        return None;
    };

    let cleaned_path = path.trim_start_matches('/').trim_end_matches('/');
    let cleaned_path = cleaned_path.strip_suffix(".git").unwrap_or(cleaned_path);
    let segments: Vec<&str> = cleaned_path.split('/').collect();
    if segments.len() < 2 {
        return None;
    }
    let owner = segments[0].to_string();
    // For nested GitLab groups (`group/subgroup/repo`), the repo is
    // the last segment and the owner is the first; the middle
    // segments belong to the URL but the consumer only needs
    // owner/repo for the compose helpers.
    let repo = segments[segments.len() - 1].to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }

    let host_lower = host.to_lowercase();
    let kind = classify_host(&host_lower);

    Some(ParsedRemote {
        host: kind,
        host_str: host,
        owner,
        repo,
        original: trimmed.to_string(),
    })
}

fn classify_host(host: &str) -> RemoteHost {
    if host == "github.com" {
        RemoteHost::Github
    } else if host == "gitlab.com" {
        RemoteHost::Gitlab
    } else if host.starts_with("gitlab.") {
        // Common "gitlab.example.com" pattern. Caller can use the
        // host_str to compose the URL.
        RemoteHost::GitlabSelfHosted(host.to_string())
    } else if host == "bitbucket.org" || host.ends_with(".bitbucket.org") {
        RemoteHost::Bitbucket
    } else if host == "gitea.com" || host.starts_with("gitea.") {
        RemoteHost::Gitea
    } else {
        RemoteHost::Other(host.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(url: &str) -> ParsedRemote {
        parse_remote_url(url).unwrap_or_else(|| panic!("expected parse for {url}"))
    }

    #[test]
    fn ssh_github_with_dot_git() {
        let p = parsed("git@github.com:owner/repo.git");
        assert_eq!(p.host, RemoteHost::Github);
        assert_eq!(p.host_str, "github.com");
        assert_eq!(p.owner, "owner");
        assert_eq!(p.repo, "repo");
    }

    #[test]
    fn ssh_github_without_dot_git() {
        let p = parsed("git@github.com:owner/repo");
        assert_eq!(p.host, RemoteHost::Github);
        assert_eq!(p.repo, "repo");
    }

    #[test]
    fn https_github() {
        let p = parsed("https://github.com/acme/widgets.git");
        assert_eq!(p.host, RemoteHost::Github);
        assert_eq!(p.owner, "acme");
        assert_eq!(p.repo, "widgets");
    }

    #[test]
    fn gitlab_dot_com() {
        let p = parsed("https://gitlab.com/group/repo.git");
        assert_eq!(p.host, RemoteHost::Gitlab);
    }

    #[test]
    fn gitlab_self_hosted_via_gitlab_prefix() {
        let p = parsed("git@gitlab.example.com:group/repo.git");
        assert_eq!(
            p.host,
            RemoteHost::GitlabSelfHosted("gitlab.example.com".into())
        );
    }

    #[test]
    fn bitbucket_org() {
        let p = parsed("git@bitbucket.org:team/repo.git");
        assert_eq!(p.host, RemoteHost::Bitbucket);
    }

    #[test]
    fn gitea_dot_com() {
        let p = parsed("https://gitea.com/owner/repo.git");
        assert_eq!(p.host, RemoteHost::Gitea);
    }

    #[test]
    fn gitea_self_hosted_via_gitea_prefix() {
        let p = parsed("https://gitea.internal.example.com/owner/repo.git");
        assert_eq!(p.host, RemoteHost::Gitea);
    }

    #[test]
    fn unknown_host_falls_into_other() {
        let p = parsed("https://code.example.com/owner/repo.git");
        assert!(matches!(p.host, RemoteHost::Other(_)));
        if let RemoteHost::Other(h) = p.host {
            assert_eq!(h, "code.example.com");
        }
    }

    #[test]
    fn nested_gitlab_groups_keep_first_owner_and_last_repo() {
        let p = parsed("https://gitlab.com/group/subgroup/repo.git");
        assert_eq!(p.owner, "group");
        assert_eq!(p.repo, "repo");
    }

    #[test]
    fn https_with_user_and_port_strips_both() {
        let p = parsed("https://user@github.com:443/owner/repo.git");
        assert_eq!(p.host, RemoteHost::Github);
        assert_eq!(p.host_str, "github.com");
    }

    #[test]
    fn ssh_url_scheme_is_supported() {
        let p = parsed("ssh://git@gitlab.com/group/repo.git");
        assert_eq!(p.host, RemoteHost::Gitlab);
    }

    #[test]
    fn git_protocol_is_supported() {
        let p = parsed("git://github.com/owner/repo.git");
        assert_eq!(p.host, RemoteHost::Github);
    }

    #[test]
    fn url_is_normalized_to_lowercase_for_classification_only() {
        // Classification matches case-insensitively; the host_str
        // preserves the original casing so the rendered URL keeps
        // whatever the user typed.
        let p = parsed("git@GitHub.com:owner/repo.git");
        assert_eq!(p.host, RemoteHost::Github);
        assert_eq!(p.host_str, "GitHub.com");
    }

    #[test]
    fn host_str_preserves_original_casing() {
        let p = parsed("https://GITHUB.COM/owner/repo");
        assert_eq!(p.host_str, "GITHUB.COM");
        assert_eq!(p.host, RemoteHost::Github);
    }

    #[test]
    fn empty_string_returns_none() {
        assert!(parse_remote_url("").is_none());
        assert!(parse_remote_url("   ").is_none());
    }

    #[test]
    fn unknown_scheme_returns_none() {
        assert!(parse_remote_url("ftp://example.com/owner/repo").is_none());
    }

    #[test]
    fn missing_path_segments_returns_none() {
        assert!(parse_remote_url("https://github.com/owner").is_none());
        assert!(parse_remote_url("git@github.com:owner").is_none());
    }

    #[test]
    fn trailing_slashes_are_tolerated() {
        let p = parsed("https://github.com/owner/repo/");
        assert_eq!(p.repo, "repo");
    }

    #[test]
    fn preserves_original_url_for_fallback() {
        let url = "git@github.com:owner/repo.git";
        let p = parsed(url);
        assert_eq!(p.original, url);
    }
}
