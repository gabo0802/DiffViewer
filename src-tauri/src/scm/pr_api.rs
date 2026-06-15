use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestSummary {
    pub id: String, // Github number or Gitlab iid
    pub title: String,
    pub state: String,
    pub source_branch: String,
    pub target_branch: String,
    pub author: String,
}

pub enum RepoHost {
    GitHub,
    GitLab(String), // The host URL
}

pub struct RepoInfo {
    pub host: RepoHost,
    pub owner: String,
    pub repo: String,
}

pub fn parse_remote_url(url: &str, gitlab_host_url: Option<&str>) -> Option<RepoInfo> {
    // Strip trailing .git
    let url = url.strip_suffix(".git").unwrap_or(url);

    // E.g. git@github.com:owner/repo
    if url.starts_with("git@") {
        let parts: Vec<&str> = url.split(':').collect();
        if parts.len() != 2 {
            return None;
        }
        let host_part = parts[0].strip_prefix("git@").unwrap();
        let path_part = parts[1];
        let path_segments: Vec<&str> = path_part.split('/').collect();
        if path_segments.len() < 2 {
            return None;
        }
        let owner = path_segments[0..path_segments.len() - 1].join("/");
        let repo = path_segments.last().unwrap().to_string();

        let host = if host_part == "github.com" {
            RepoHost::GitHub
        } else if host_part == "gitlab.com" {
            RepoHost::GitLab("https://gitlab.com".to_string())
        } else if let Some(custom) = gitlab_host_url {
            if custom.contains(host_part) {
                RepoHost::GitLab(custom.to_string())
            } else {
                return None;
            }
        } else {
            return None;
        };

        return Some(RepoInfo { host, owner, repo });
    }

    // E.g. https://github.com/owner/repo
    if url.starts_with("http://") || url.starts_with("https://") {
        let parsed_url = url::Url::parse(url).ok()?;
        let host_str = parsed_url.host_str()?;
        let path = parsed_url.path().trim_start_matches('/');
        let path_segments: Vec<&str> = path.split('/').collect();
        if path_segments.len() < 2 {
            return None;
        }
        let owner = path_segments[0..path_segments.len() - 1].join("/");
        let repo = path_segments.last().unwrap().to_string();

        let host = if host_str == "github.com" {
            RepoHost::GitHub
        } else if host_str == "gitlab.com" {
            RepoHost::GitLab("https://gitlab.com".to_string())
        } else if let Some(custom) = gitlab_host_url {
            if custom.contains(host_str) {
                RepoHost::GitLab(custom.to_string())
            } else {
                return None;
            }
        } else {
            return None;
        };

        return Some(RepoInfo { host, owner, repo });
    }

    None
}

pub fn get_pull_requests(
    repo_info: &RepoInfo,
    github_pat: Option<&str>,
    gitlab_pat: Option<&str>,
) -> Result<Vec<PullRequestSummary>, String> {
    match &repo_info.host {
        RepoHost::GitHub => get_github_prs(repo_info, github_pat),
        RepoHost::GitLab(host_url) => get_gitlab_mrs(repo_info, host_url, gitlab_pat),
    }
}

fn get_github_prs(
    repo_info: &RepoInfo,
    pat: Option<&str>,
) -> Result<Vec<PullRequestSummary>, String> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/pulls?state=all&per_page=50",
        repo_info.owner, repo_info.repo
    );

    let mut request = ureq::get(&url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "DiffViewer");

    if let Some(token) = pat {
        if !token.is_empty() {
            request = request.header("Authorization", &format!("Bearer {}", token));
        }
    }

    let response = request
        .call()
        .map_err(|e| format!("GitHub API error: {}", e))?;

    #[derive(Deserialize)]
    struct GithubUser {
        login: String,
    }

    #[derive(Deserialize)]
    struct GithubRef {
        ref_name: Option<String>,
        #[serde(rename = "ref")]
        ref_field: Option<String>,
    }

    #[derive(Deserialize)]
    struct GithubPR {
        number: u64,
        title: String,
        state: String,
        head: GithubRef,
        base: GithubRef,
        user: GithubUser,
    }

    let text = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read GitHub response: {}", e))?;
    let prs: Vec<GithubPR> = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse GitHub response: {}", e))?;

    Ok(prs
        .into_iter()
        .map(|pr| PullRequestSummary {
            id: pr.number.to_string(),
            title: pr.title,
            state: pr.state,
            source_branch: pr.head.ref_name.or(pr.head.ref_field).unwrap_or_default(),
            target_branch: pr.base.ref_name.or(pr.base.ref_field).unwrap_or_default(),
            author: pr.user.login,
        })
        .collect())
}

fn get_gitlab_mrs(
    repo_info: &RepoInfo,
    host_url: &str,
    pat: Option<&str>,
) -> Result<Vec<PullRequestSummary>, String> {
    let encoded_project = format!("{}%2F{}", repo_info.owner, repo_info.repo);
    let url = format!(
        "{}/api/v4/projects/{}/merge_requests?state=all&per_page=50",
        host_url.trim_end_matches('/'),
        encoded_project
    );

    let mut request = ureq::get(&url).header("User-Agent", "DiffViewer");

    if let Some(token) = pat {
        if !token.is_empty() {
            request = request.header("PRIVATE-TOKEN", token);
        }
    }

    let response = request
        .call()
        .map_err(|e| format!("GitLab API error: {}", e))?;

    #[derive(Deserialize)]
    struct GitlabUser {
        username: String,
    }

    #[derive(Deserialize)]
    struct GitlabMR {
        iid: u64,
        title: String,
        state: String,
        source_branch: String,
        target_branch: String,
        author: GitlabUser,
    }

    let text = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read GitLab response: {}", e))?;
    let mrs: Vec<GitlabMR> = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse GitLab response: {}", e))?;

    Ok(mrs
        .into_iter()
        .map(|mr| PullRequestSummary {
            id: mr.iid.to_string(),
            title: mr.title,
            state: mr.state,
            source_branch: mr.source_branch,
            target_branch: mr.target_branch,
            author: mr.author.username,
        })
        .collect())
}
