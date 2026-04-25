//! Resolve commit counts via `git rev-list --count` on local bare clones.
//! Results are cached in the `commit_counts` database table.
//! In-flight resolution is deduplicated via a per-repo lock set.

#[cfg(feature = "server")]
use std::collections::{HashMap, HashSet};
#[cfg(feature = "server")]
use std::path::PathBuf;
#[cfg(feature = "server")]
use std::sync::OnceLock;
#[cfg(feature = "server")]
use tokio::sync::Mutex;

#[cfg(feature = "server")]
struct RepoCache {
    repo: &'static str,
    clone_path: PathBuf,
    git_url: String,
    /// Guards the initial clone so concurrent callers don't race.
    cloned: tokio::sync::OnceCell<()>,
    /// SHAs currently being resolved — prevents duplicate in-flight lookups.
    in_flight: Mutex<HashSet<String>>,
}

#[cfg(feature = "server")]
impl RepoCache {
    fn new(repo: &'static str, clone_path: PathBuf, git_url: String) -> Self {
        Self {
            repo,
            clone_path,
            git_url,
            cloned: tokio::sync::OnceCell::new(),
            in_flight: Mutex::new(HashSet::new()),
        }
    }

    async fn ensure_clone(&self) {
        self.cloned
            .get_or_init(|| async {
                if self.clone_path.exists() {
                    return;
                }
                if let Some(parent) = self.clone_path.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                tracing::info!(
                    "cloning {} -> {} (first run, this may take a while for large repos)",
                    self.git_url,
                    self.clone_path.display()
                );
                match tokio::process::Command::new("git")
                    .args(["clone", "--mirror", &self.git_url])
                    .arg(&self.clone_path)
                    .output()
                    .await
                {
                    Ok(out) if out.status.success() => {
                        tracing::info!("cloned {} -> {}", self.git_url, self.clone_path.display());
                    }
                    Ok(out) => {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        tracing::error!("git clone failed: {stderr}");
                    }
                    Err(e) => tracing::error!("git clone error: {e}"),
                }
            })
            .await;
    }

    async fn fetch(&self) {
        if !self.clone_path.exists() {
            return;
        }
        tracing::info!("fetching {}", self.repo);
        match tokio::process::Command::new("git")
            .args(["remote", "update", "--prune"])
            .current_dir(&self.clone_path)
            .output()
            .await
        {
            Ok(out) if out.status.success() => {
                tracing::info!("fetched {}", self.repo);
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                tracing::warn!("git fetch {} failed: {stderr}", self.repo);
            }
            Err(e) => tracing::warn!("git fetch {} error: {e}", self.repo),
        }
    }

    async fn gc(&self) {
        if !self.clone_path.exists() {
            return;
        }
        match tokio::process::Command::new("git")
            .args(["gc", "--auto", "--quiet"])
            .current_dir(&self.clone_path)
            .output()
            .await
        {
            Ok(out) if out.status.success() => {
                tracing::debug!("git gc {}", self.repo);
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                tracing::warn!("git gc {} failed: {stderr}", self.repo);
            }
            Err(e) => tracing::warn!("git gc {} error: {e}", self.repo),
        }
    }

    async fn get_or_fetch(&self, shas: &HashSet<String>) -> HashMap<String, u64> {
        self.ensure_clone().await;

        // Strip "-dirty" suffixes so the lookup uses a clean SHA,
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
            let mut join_set = tokio::task::JoinSet::new();
            for sha in &claimed {
                let sha = sha.clone();
                let clone_path = self.clone_path.clone();
                join_set.spawn(async move {
                    let output = tokio::process::Command::new("git")
                        .args(["rev-list", "--count", &sha])
                        .current_dir(&clone_path)
                        .output()
                        .await
                        .ok();
                    let count = output.and_then(|o| {
                        if o.status.success() {
                            String::from_utf8_lossy(&o.stdout).trim().parse().ok()
                        } else {
                            None
                        }
                    });
                    (sha, count)
                });
            }

            let mut resolved = Vec::new();
            while let Some(Ok(item)) = join_set.join_next().await {
                resolved.push(item);
            }

            // 4. If any SHAs failed, fetch new commits and retry once.
            let failed: Vec<String> = resolved
                .iter()
                .filter(|(_, c)| c.is_none())
                .map(|(sha, _)| sha.clone())
                .collect();
            if !failed.is_empty() {
                self.fetch().await;
                let mut retry_set = tokio::task::JoinSet::new();
                for sha in &failed {
                    let sha = sha.clone();
                    let clone_path = self.clone_path.clone();
                    retry_set.spawn(async move {
                        let output = tokio::process::Command::new("git")
                            .args(["rev-list", "--count", &sha])
                            .current_dir(&clone_path)
                            .output()
                            .await
                            .ok();
                        let count = output.and_then(|o| {
                            if o.status.success() {
                                String::from_utf8_lossy(&o.stdout).trim().parse().ok()
                            } else {
                                None
                            }
                        });
                        (sha, count)
                    });
                }
                while let Some(Ok(item)) = retry_set.join_next().await {
                    // Overwrite the failed entry with the retry result.
                    if let Some(pos) = resolved.iter().position(|(s, _)| *s == item.0) {
                        resolved[pos] = item;
                    }
                }
            }

            // 5. Store results in DB and populate return map.
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

            // 6. Release in-flight claims.
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
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT sha, count FROM commit_counts WHERE repo = $1 AND sha = ANY($2)",
        )
        .bind(self.repo)
        .bind(shas)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
        rows.into_iter().map(|(sha, c)| (sha, c as u64)).collect()
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

// ── mac-mgmt ───────────────────────────────────────────────────────────

#[cfg(feature = "server")]
static MAC_MGMT: OnceLock<RepoCache> = OnceLock::new();

/// Fetch commit counts for mac-mgmt SHAs.
#[cfg(feature = "server")]
pub async fn mac_mgmt_commit_counts(shas: &HashSet<String>) -> HashMap<String, u64> {
    let cfg = &crate::config::config().git;
    MAC_MGMT
        .get_or_init(|| {
            RepoCache::new(
                "mac-mgmt",
                PathBuf::from(&cfg.state_dir).join("repos/mac-mgmt.git"),
                cfg.mac_mgmt_url.clone(),
            )
        })
        .get_or_fetch(shas)
        .await
}

// ── nixpkgs ────────────────────────────────────────────────────────────

#[cfg(feature = "server")]
static NIXPKGS: OnceLock<RepoCache> = OnceLock::new();

/// Fetch commit counts for nixpkgs SHAs.
#[cfg(feature = "server")]
pub async fn nixpkgs_commit_counts(shas: &HashSet<String>) -> HashMap<String, u64> {
    let cfg = &crate::config::config().git;
    NIXPKGS
        .get_or_init(|| {
            RepoCache::new(
                "nixpkgs",
                PathBuf::from(&cfg.state_dir).join("repos/nixpkgs.git"),
                cfg.nixpkgs_url.clone(),
            )
        })
        .get_or_fetch(shas)
        .await
}

// ── Background fetch loop ──────────────────────────────────────────────

#[cfg(feature = "server")]
pub fn spawn_fetch_loop() {
    let interval_secs = crate::config::config().git.fetch_interval_secs;
    tokio::spawn(async move {
        let fetch_interval = std::time::Duration::from_secs(interval_secs);
        // Run git gc every 6 hours.
        let gc_every = std::time::Duration::from_secs(6 * 3600);
        let mut last_gc = tokio::time::Instant::now();
        loop {
            tokio::time::sleep(fetch_interval).await;
            for cache in [MAC_MGMT.get(), NIXPKGS.get()].into_iter().flatten() {
                cache.fetch().await;
            }
            if last_gc.elapsed() >= gc_every {
                last_gc = tokio::time::Instant::now();
                for cache in [MAC_MGMT.get(), NIXPKGS.get()].into_iter().flatten() {
                    cache.gc().await;
                }
            }
        }
    });
}
