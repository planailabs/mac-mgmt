//! Fetch commit counts from GitLab APIs via binary search.
//! Results are cached permanently (commit count for a SHA never changes).

#[cfg(feature = "server")]
use std::collections::{HashMap, HashSet};
#[cfg(feature = "server")]
use std::sync::OnceLock;
#[cfg(feature = "server")]
use tokio::sync::Mutex;

#[cfg(feature = "server")]
struct RepoCache {
    base_url: &'static str,
    cache: Mutex<HashMap<String, u64>>,
}

#[cfg(feature = "server")]
impl RepoCache {
    fn new(base_url: &'static str) -> Self {
        Self {
            base_url,
            cache: Mutex::new(HashMap::new()),
        }
    }

    async fn get_or_fetch(&self, shas: &HashSet<String>) -> HashMap<String, u64> {
        let client = reqwest::Client::new();
        let mut result = HashMap::new();
        let mut to_fetch = Vec::new();

        {
            let cached = self.cache.lock().await;
            for sha in shas {
                if let Some(&count) = cached.get(sha) {
                    result.insert(sha.clone(), count);
                } else {
                    to_fetch.push(sha.clone());
                }
            }
        }

        for sha in &to_fetch {
            if let Some(count) = gitlab_binary_search(&client, self.base_url, sha).await {
                result.insert(sha.clone(), count);
                self.cache.lock().await.insert(sha.clone(), count);
            }
        }

        result
    }
}

/// Binary search the GitLab commits endpoint to find the total commit count.
/// Uses per_page=1 and checks if the page has data (~17 requests max for 100K commits).
#[cfg(feature = "server")]
async fn gitlab_binary_search(
    client: &reqwest::Client,
    base_url: &str,
    sha: &str,
) -> Option<u64> {
    let page_has_data = |page: u64| {
        let url = format!("{base_url}?ref_name={sha}&per_page=1&page={page}");
        let client = client.clone();
        async move {
            let resp = client
                .get(&url)
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await
                .ok()?;
            if !resp.status().is_success() {
                return None;
            }
            let next = resp
                .headers()
                .get("x-next-page")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if !next.is_empty() {
                return Some(true);
            }
            let body = resp.text().await.ok()?;
            Some(body.len() > 2)
        }
    };

    if !page_has_data(1).await? {
        return None;
    }

    let mut lo: u64 = 1;
    let mut hi: u64 = 100_000;

    while lo < hi {
        let mid = lo + (hi - lo + 1) / 2;
        if page_has_data(mid).await? {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }

    Some(lo)
}

// ── mac-mgmt ───────────────────────────────────────────────────────────

#[cfg(feature = "server")]
static MAC_MGMT: OnceLock<RepoCache> = OnceLock::new();

/// Fetch commit counts for mac-mgmt SHAs (git.plan.ai).
#[cfg(feature = "server")]
pub async fn mac_mgmt_commit_counts(shas: &HashSet<String>) -> HashMap<String, u64> {
    MAC_MGMT
        .get_or_init(|| {
            RepoCache::new(
                "https://git.plan.ai/api/v4/projects/plan-ai%2Fmac-mgmt/repository/commits",
            )
        })
        .get_or_fetch(shas)
        .await
}

// ── nixpkgs ────────────────────────────────────────────────────────────

#[cfg(feature = "server")]
static NIXPKGS: OnceLock<RepoCache> = OnceLock::new();

/// Fetch commit counts for nixpkgs SHAs (git.plan.ai/plan-ai/nixpkgs).
#[cfg(feature = "server")]
pub async fn nixpkgs_commit_counts(shas: &HashSet<String>) -> HashMap<String, u64> {
    NIXPKGS
        .get_or_init(|| {
            RepoCache::new(
                "https://git.plan.ai/api/v4/projects/plan-ai%2Fnixpkgs/repository/commits",
            )
        })
        .get_or_fetch(shas)
        .await
}
