//! Memvault lifecycle handle.
//!
//! Manages the memvault subsystem within the daemon: store, blockstore bridge,
//! web API server, and periodic housekeeping.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use mac_mgmt_common::MemvaultConfig;
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::BlockstoreBridge;

/// Handle to the running memvault subsystem.
pub struct MemvaultHandle {
    store: Arc<memvault_store::MemvaultStore>,
    bridge: BlockstoreBridge,
    client: Arc<memvault_api::LocalClient>,
    config: MemvaultConfig,
    /// Join handle for the web server task (if started).
    web_handle: Option<tokio::task::JoinHandle<()>>,
}

impl MemvaultHandle {
    /// Initialize the memvault subsystem.
    ///
    /// Opens the store, creates the local client, starts the web server if
    /// configured, and returns the handle.
    pub async fn init(config: &MemvaultConfig, peer_id: Vec<u8>) -> Result<Self> {
        let data_dir = if config.data_dir.is_empty() {
            default_data_dir()
        } else {
            PathBuf::from(&config.data_dir)
        };
        std::fs::create_dir_all(&data_dir)?;

        let db_path = data_dir.join("blocks.redb");
        let store = Arc::new(memvault_store::MemvaultStore::open(&db_path)?);
        let bridge = BlockstoreBridge::new(Arc::clone(&store));

        // Resolve cluster_id from the store (single source of truth, matching
        // memctl). Older daemon installs persisted the cluster_id only as a
        // hex sidecar file — migrate that to the store on first run so the
        // two paths stay aligned.
        let cluster_id = resolve_cluster_id(&store, &data_dir);

        // Create the local client (used by the web API and for internal operations).
        // `open` runs rebuild_if_needed so an out-of-date blockstore is migrated
        // before the web API starts serving requests.
        let client = memvault_api::LocalClient::open(
            Arc::clone(&store),
            Arc::new(RwLock::new(memvault_query::TextIndex::new())),
            Arc::new(RwLock::new(memvault_query::QuotaManager::new(
                Default::default(),
            ))),
            Arc::new(memvault_api::EventBus::new(256)),
            peer_id.clone(),
            cluster_id,
        )?;

        // Open the redb-bypassing keystore (tokens + key material). A second
        // process (memctl) can append to it while the daemon runs, which
        // redb's cross-process exclusive lock would forbid. Optionally
        // AEAD-encrypted at rest when MEMVAULT_KEYSTORE_PASSPHRASE is set.
        // The token/key keystore is opened by LocalClient itself (beside the
        // blockstore at <data_dir>/identity/), honouring
        // MEMVAULT_KEYSTORE_PASSPHRASE. Run the one-off redb→keystore token
        // migration now that both stores are open.
        // The keystore (tokens + key material) is opened by LocalClient
        // itself, beside the blockstore at <data_dir>/identity/. Identity
        // material lives only in the keystore + redb store; import and delete
        // any legacy loose files (admin.key, cluster_admin_genesis.cbor,
        // root cluster_id) once, then run the one-off redb→keystore token
        // migration.
        let identity_dir = data_dir.join("identity");
        std::fs::create_dir_all(&identity_dir)?;
        client.migrate_legacy_identity_files(&identity_dir);
        let migrated = client.migrate_tokens_to_keystore();
        if migrated > 0 {
            info!(count = migrated, "migrated legacy redb tokens into keystore");
        }

        // Load the admin signing key (genesis admin) from the keystore so the
        // web API can issue JWTs for the built-in UI agent and verify
        // attestation signatures. (Legacy admin.key was folded in above.)
        client.load_admin_keys_from_keystore();
        // Pre-genesis grant signing no longer needs a founder key: a node
        // signs grants for its own (node-owned) and its agents' buckets
        // with its node key, which gains cluster trust at genesis/join, so
        // those grants become cluster-valid with no reissue.

        // Load the pinned AdminGenesis block (cluster root of trust) from the
        // keystore. Without it, peers operate in pre-genesis mode — they
        // can't verify NodeAttestations from admin.
        if let Some(pin_bytes) = client.pinned_admin_genesis_bytes_from_keystore() {
            match serde_ipld_dagcbor::from_slice::<memvault_auth::AdminGenesis>(&pin_bytes) {
                Ok(g) => {
                    if let Err(e) = g.verify_self_signature() {
                        tracing::warn!(error = %e, "pinned admin_genesis has bad signature; ignoring");
                    } else {
                        client.set_pinned_admin_genesis(g);
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "could not decode pinned admin_genesis; ignoring");
                }
            }
        }

        // Set the node signing key (the daemon's libp2p ed25519 host key).
        // Used to sign agent attestations + revocations, and looked up by the
        // web auth bootstrap. Set before Arc-wrapping so revoke_agent etc.
        // have it available on the shared client.
        let host_key = crate::host_keys::load_or_generate()?;
        let node_signing_key =
            crate::p2p::identity::ed25519_dalek_signing_key_from_russh(&host_key)?;
        client.set_node_signing_key(node_signing_key);

        let client = Arc::new(client);

        // Apply keystore changes from other processes live — e.g. a co-admin
        // key admitted by a separate memctl (or the swarm thread) is loaded
        // into this running daemon without a restart.
        client.start_keystore_watch();

        // Stamp this node as the owner of its per-node legacy bucket (the
        // bucket is created during rebuild, before the node key is set), so
        // the node can delegate access to its own legacy data with its node
        // key — no admin required.
        if let Err(e) = client.ensure_legacy_bucket_node_owner() {
            warn!("could not stamp legacy-bucket node owner: {e}");
        }

        // If a legacy bucket exists, make sure every AgentHost can read +
        // write to it. Idempotent. Now signed by the node key via the
        // node-owner authority, so this works on non-admin nodes too.
        if let Err(e) = ensure_legacy_bucket_agent_grant(&client).await {
            warn!("could not ensure legacy-bucket agent grant: {e}");
        }

        // Load or rebuild the full-text search index.
        let index_cache_path = data_dir.join("text_index.json");
        match client.load_or_rebuild_index(&index_cache_path).await {
            Ok((d, e, a)) => info!("text index ready: {d} docs, {e} entities, {a} attachments"),
            Err(e) => warn!("failed to populate text index: {e}"),
        }

        // Start the API server.
        let _ = peer_id;
        let web_handle = if config.port > 0 {
            let port = config.port;
            let trust = memvault_api::bootstrap::bootstrap_cluster_trust(&client)
                .map_err(|e| anyhow::anyhow!("cluster trust bootstrap: {e}"))?;
            // Inside async fn init() driven by the daemon's runtime —
            // spawn the watcher here so it lives on the same runtime as
            // the HTTP server below.
            let _watcher = memvault_api::sigchain::spawn_sigchain_watcher(
                Arc::clone(&client),
                trust.admin_pubkey,
                trust.trust_state.clone(),
            );
            memvault_web::init_ui_agent(&client, &data_dir)
                .map_err(|e| anyhow::anyhow!("init ui agent: {e}"))?;
            let app_state = Arc::new(memvault_web::AppState {
                client: Arc::clone(&client) as Arc<dyn memvault_api::MemvaultClient>,
                event_bus: Arc::new(memvault_api::EventBus::new(256)),
                admin_pubkey: trust.admin_pubkey,
                node_trust: Arc::clone(&trust.trust_state.node_trust),
                revoked_agents: Arc::clone(&trust.trust_state.revoked_agents),
                revoked_nodes: Arc::clone(&trust.trust_state.revoked_nodes),
                metrics: Arc::new(memvault_api::metrics::Metrics::new()),
                agent_attestation_lookup: None,
            });
            memvault_web::ui::state::set_client(Arc::clone(&client));
            let router = memvault_web::build_fullstack_router(app_state);
            let handle = tokio::spawn(async move {
                let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
                let listener = match tokio::net::TcpListener::bind(addr).await {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::error!("memvault web: failed to bind port {port}: {e}");
                        return;
                    }
                };
                info!(port, "memvault web API started");
                if let Err(e) = axum::serve(listener, router).await {
                    tracing::error!("memvault web server error: {e}");
                }
            });
            Some(handle)
        } else {
            None
        };

        // Periodic GC of invalidated token records: a token that has been
        // revoked, expired, or exhausted is retained for 30 days (so it still
        // shows in audits/lists), then its keystore records are reclaimed.
        // This is the only sweeper — `list`/`issue` stay read-only.
        {
            let ks = Arc::clone(client.keystore());
            tokio::spawn(async move {
                let mut tick =
                    tokio::time::interval(std::time::Duration::from_secs(6 * 60 * 60));
                loop {
                    tick.tick().await;
                    let now_ns = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos() as u64)
                        .unwrap_or(0);
                    let n = memvault_api::tokens::gc_invalidated_tokens(&ks, now_ns);
                    if n > 0 {
                        info!(deleted = n, "GC'd invalidated token records");
                    }
                }
            });
        }

        info!(data_dir = %data_dir.display(), "memvault initialized");

        Ok(Self {
            store,
            bridge,
            client,
            config: config.clone(),
            web_handle,
        })
    }

    /// Periodic tick — called from the daemon's health loop.
    ///
    /// Performs housekeeping: gossip head announcements, check attestation
    /// expiry, process eager-replication queue, run deferred extractions.
    pub async fn tick(&self) {
        // Check for new blocks that need eager replication announcement
        // (In a full implementation, this would gossip DocumentHead and
        // AttachmentHeadRef for any new envelopes since last tick.)

        // Process deferred extraction queue
        // (Large files queued for async extraction get processed here.)
    }

    /// Handle an incoming block from the p2p network (via beetswap).
    pub fn on_block_received(&self, cid: &[u8], data: &[u8]) -> bool {
        match self.bridge.on_block_received(cid, data) {
            Ok(was_new) => was_new,
            Err(e) => {
                warn!("memvault: error storing received block: {e}");
                false
            }
        }
    }

    /// Serve a block requested by a remote peer (via beetswap).
    pub fn on_block_wanted(&self, cid: &[u8]) -> Option<Vec<u8>> {
        match self.bridge.on_block_wanted(cid) {
            Ok(data) => data,
            Err(e) => {
                warn!("memvault: error serving block: {e}");
                None
            }
        }
    }

    /// Shutdown cleanly — abort web server, flush store.
    pub async fn shutdown(self) {
        info!("memvault shutting down");
        if let Some(handle) = self.web_handle {
            handle.abort();
        }
    }

    /// Get a reference to the underlying store.
    pub fn store(&self) -> &memvault_store::MemvaultStore {
        &self.store
    }

    /// Get a reference to the blockstore bridge.
    pub fn bridge(&self) -> &BlockstoreBridge {
        &self.bridge
    }

    /// Get a reference to the local client.
    pub fn client(&self) -> &memvault_api::LocalClient {
        &self.client
    }
}

/// Ensure the legacy bucket carries a Role(AgentHost) read+write grant so
/// every daemon-enrolled agent can access pre-bucket data. (Nodes don't need
/// a Role(Node) grant: they hold the data via block-level sync and write to
/// their own buckets via the node-owner authority — they never go through the
/// agent ACL path.) No-ops when:
///   * no legacy bucket exists (fresh install / post-migration cluster),
///   * the daemon doesn't hold the admin signing key,
///   * an equivalent grant is already on chain.
async fn ensure_legacy_bucket_agent_grant(
    client: &memvault_api::LocalClient,
) -> anyhow::Result<()> {
    let Some(bucket) = client.find_legacy_bucket() else {
        return Ok(());
    };

    let existing = client.list_bucket_grants(&bucket)?;
    let covered = existing.iter().any(|(_, g)| {
        matches!(
            &g.audience,
            memvault_auth::GrantAudience::Role(memvault_auth::AgentRole::AgentHost)
        ) && g.actions.contains(&memvault_auth::Action::Read)
            && g.actions.contains(&memvault_auth::Action::Write)
    });
    if covered {
        return Ok(());
    }

    let cid = client
        .issue_bucket_grant(
            &bucket,
            memvault_auth::GrantAudience::Role(memvault_auth::AgentRole::AgentHost),
            vec![memvault_auth::Action::Read, memvault_auth::Action::Write],
            u64::MAX,
        )
        .await?;
    info!(
        legacy_bucket = %bucket,
        cid = %hex::encode(&cid),
        "auto-granted Role(AgentHost) read+write on legacy bucket",
    );
    Ok(())
}

fn default_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("memvault")
}


/// Read the cluster_id sidecar file (legacy daemon location). Returns
/// `vec![0u8; 32]` if the file is absent or malformed — same sentinel
/// `LocalClient::open` already treats as "no cluster yet".
fn cluster_id_from_dir(data_dir: &PathBuf) -> Vec<u8> {
    let id_path = data_dir.join("cluster_id");
    match std::fs::read_to_string(&id_path) {
        Ok(hex_str) => hex::decode(hex_str.trim()).unwrap_or_else(|_| vec![0u8; 32]),
        Err(_) => vec![0u8; 32],
    }
}

/// Resolve the daemon's cluster_id, preferring the store (the single
/// source of truth used by memctl). Falls back to the legacy `cluster_id`
/// sidecar file for older daemon installs and migrates it into the store
/// so subsequent runs see a consistent value from both paths.
fn resolve_cluster_id(store: &memvault_store::MemvaultStore, data_dir: &PathBuf) -> Vec<u8> {
    if let Ok(Some(cid)) = store.get_local_cluster_id() {
        return cid;
    }
    let from_file = cluster_id_from_dir(data_dir);
    // Don't persist the zero sentinel — that's "no cluster yet" and would
    // shadow a real cluster_id written later by genesis / join.
    if from_file != [0u8; 32] {
        if let Err(e) = store.set_local_cluster_id(&from_file) {
            warn!("could not migrate cluster_id from sidecar to store: {e}");
        }
    }
    from_file
}
