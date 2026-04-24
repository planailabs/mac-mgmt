//! Fetch commit counts from GitLab APIs via binary search.
//! Results are cached in the database so they survive restarts.
//! In-flight resolution is deduplicated via a per-repo lock set.

#[cfg(feature = "server")]
use std::collections::{HashMap, HashSet};
#[cfg(feature = "server")]
use std::sync::OnceLock;
#[cfg(feature = "server")]
use tokio::sync::Mutex;

#[cfg(feature = "server")]
struct RepoCache {
    repo: &'static str,
    base_url: &'static str,
    /// Lower bound for binary search (optimization for large repos).
    search_lo: u64,
    /// Upper bound for binary search.
    search_hi: u64,
    /// SHAs currently being resolved — prevents duplicate in-flight lookups.
    in_flight: Mutex<HashSet<String>>,
}

#[cfg(feature = "server")]
impl RepoCache {
    fn new(repo: &'static str, base_url: &'static str, search_lo: u64, search_hi: u64) -> Self {
        Self {
            repo,
            base_url,
            search_lo,
            search_hi,
            in_flight: Mutex::new(HashSet::new()),
        }
    }

    async fn get_or_fetch(&self, shas: &HashSet<String>) -> HashMap<String, u64> {
        // Strip "-dirty" suffixes so the API lookup uses a clean SHA,
        // then map results back to the original keys.
        let mut clean_to_orig: HashMap<String, Vec<String>> = HashMap::new();
        for sha in shas {
            let clean = sha.strip_suffix("-dirty").unwrap_or(sha).to_string();
            clean_to_orig.entry(clean).or_default().push(sha.clone());
        }

        let clean_shas: Vec<String> = clean_to_orig.keys().cloned().collect();
        let mut result = HashMap::new();

        // 1. Check database for cached counts.
        let db_hits = self.db_lookup(&clean_shas).await;
        let mut to_fetch = Vec::new();
        for clean in &clean_shas {
            if let Some(&count) = db_hits.get(clean) {
                for orig in &clean_to_orig[clean] {
                    result.insert(orig.clone(), count);
                }
            } else {
                to_fetch.push(clean.clone());
            }
        }

        if to_fetch.is_empty() {
            return result;
        }

        // 2. Claim SHAs not already in-flight to avoid duplicate resolution.
        let claimed = {
            let mut flight = self.in_flight.lock().await;
            let mut claimed = Vec::new();
            let mut waiting = Vec::new();
            for sha in &to_fetch {
                if flight.insert(sha.clone()) {
                    claimed.push(sha.clone());
                } else {
                    waiting.push(sha.clone());
                }
            }
            drop(flight);

            // For SHAs already in-flight by another caller, wait briefly
            // then check the DB — the other caller will have stored the
            // result by then.
            if !waiting.is_empty() {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let late_hits = self.db_lookup(&waiting).await;
                for (sha, count) in late_hits {
                    if let Some(origs) = clean_to_orig.get(&sha) {
                        for orig in origs {
                            result.insert(orig.clone(), count);
                        }
                    }
                }
            }

            claimed
        };

        if !claimed.is_empty() {
            // 3. Resolve all claimed SHAs in parallel.
            let client = reqwest::Client::new();
            let mut join_set = tokio::task::JoinSet::new();
            for sha in &claimed {
                let client = client.clone();
                let sha = sha.clone();
                let base_url = self.base_url;
                let lo = self.search_lo;
                let hi = self.search_hi;
                join_set.spawn(async move {
                    let count =
                        gitlab_binary_search(&client, base_url, &sha, lo, hi).await;
                    (sha, count)
                });
            }

            let mut resolved = Vec::new();
            while let Some(Ok(item)) = join_set.join_next().await {
                resolved.push(item);
            }

            // 4. Store results in DB and populate return map.
            for (sha, count) in &resolved {
                if let Some(count) = count {
                    self.db_store(sha, *count).await;
                    if let Some(origs) = clean_to_orig.get(sha) {
                        for orig in origs {
                            result.insert(orig.clone(), *count);
                        }
                    }
                }
            }

            // 5. Release in-flight claims.
            let mut flight = self.in_flight.lock().await;
            for (sha, _) in &resolved {
                flight.remove(sha);
            }
        }

        result
    }

    async fn db_lookup(&self, shas: &[String]) -> HashMap<String, u64> {
        let Ok(pool) = crate::server_pool() else {
            return HashMap::new();
        };
        let mut result = HashMap::new();
        // sqlx doesn't support WHERE IN with slices on all backends,
        // so use a single query with ANY($1).
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT sha, count FROM commit_counts WHERE repo = $1 AND sha = ANY($2)",
        )
        .bind(self.repo)
        .bind(shas)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
        for (sha, count) in rows {
            result.insert(sha, count as u64);
        }
        result
    }

    async fn db_store(&self, sha: &str, count: u64) {
        let Ok(pool) = crate::server_pool() else {
            return;
        };
        let _ = sqlx::query(
            "INSERT INTO commit_counts (repo, sha, count) VALUES ($1, $2, $3) \
             ON CONFLICT (repo, sha) DO NOTHING",
        )
        .bind(self.repo)
        .bind(sha)
        .bind(count as i64)
        .fetch_optional(&pool)
        .await;
    }
}

/// Binary search the GitLab commits endpoint to find the total commit count.
/// Uses per_page=1 and checks if the page has data (~17 requests max for 100K commits).
#[cfg(feature = "server")]
async fn gitlab_binary_search(
    client: &reqwest::Client,
    base_url: &str,
    sha: &str,
    start_lo: u64,
    start_hi: u64,
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

    let mut lo: u64 = start_lo;
    let mut hi: u64 = start_hi;

    // If start_lo > 1, verify it has data; if not, fall back to 1.
    if lo > 1 && !page_has_data(lo).await.unwrap_or(false) {
        lo = 1;
    }

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
                "mac-mgmt",
                "https://git.plan.ai/api/v4/projects/plan-ai%2Fmac-mgmt/repository/commits",
                1,
                100_000,
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
                "nixpkgs",
                "https://git.plan.ai/api/v4/projects/plan-ai%2Fnixpkgs/repository/commits",
                900_000,
                2_000_000,
            )
        })
        .get_or_fetch(shas)
        .await
}
