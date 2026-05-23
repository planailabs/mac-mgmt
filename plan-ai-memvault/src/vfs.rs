//! Virtual filesystem layer — organises memvault nodes into a path hierarchy.
//!
//! Directories are entities with `kind = "vfs:dir"`.  Parent→child relationships
//! are edges with `relation = "vfs:child"` and a `"name"` property on the edge.
//! The root directory is tagged `(vfs, root)`.

use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use tokio::sync::OnceCell;

use crate::backend::Backend;

const VFS_DIR_KIND: &str = "vfs:dir";
const VFS_CHILD_REL: &str = "vfs:child";
const VFS_ROOT_TAG: (&str, &str) = ("vfs", "root");

/// Ensure a node ID has the `entity:` prefix exactly once.
fn ensure_entity_id(id: &str) -> String {
    let bare = id.strip_prefix("entity:").unwrap_or(id);
    format!("entity:{bare}")
}

/// Strip any type prefix, returning the bare hex ID.
fn bare_hex(id: &str) -> &str {
    id.strip_prefix("entity:").unwrap_or(id)
}

/// A single entry in a directory listing.
#[derive(Debug, Serialize)]
pub struct VfsEntry {
    pub name: String,
    pub node_id: String,
    pub node_type: &'static str,
    pub edge_id: String,
}

/// The VFS handle. Caches the root entity ID for the session.
pub struct Vfs<'a> {
    backend: &'a dyn Backend,
    root_id: OnceCell<String>,
}

impl<'a> Vfs<'a> {
    pub fn new(backend: &'a dyn Backend) -> Self {
        Self {
            backend,
            root_id: OnceCell::new(),
        }
    }

    // ── Root management ────────────────────────────────────────────

    /// Return the root entity ID, creating the root if necessary.
    pub async fn ensure_root(&self) -> Result<String> {
        self.root_id
            .get_or_try_init(|| async { self.find_or_create_root().await })
            .await
            .cloned()
    }

    async fn find_or_create_root(&self) -> Result<String> {
        let entities = self.backend.list_entities(500).await?;
        let empty = vec![];
        // Handle both: plain array (local backend) or {"nodes":[...]} (HTTP backend)
        let arr = entities
            .as_array()
            .or_else(|| entities.get("nodes").and_then(|v| v.as_array()))
            .unwrap_or(&empty);
        let mut candidates: Vec<String> = Vec::new();
        let mut fallback_candidates: Vec<String> = Vec::new();
        for e in arr {
            // Accept local ("kind") or HTTP ("node_type" == "entity") entries
            let kind_match = e.get("kind").and_then(|v| v.as_str()) == Some(VFS_DIR_KIND);
            let is_entity = e.get("node_type").and_then(|v| v.as_str()) == Some("entity");
            if !kind_match && !is_entity {
                continue;
            }
            // Get bare hex ID: "id" (local) or "node_id" with prefix (HTTP)
            let raw_id = e
                .get("id")
                .or_else(|| e.get("node_id"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let id = raw_id.strip_prefix("entity:").unwrap_or(raw_id);
            if id.is_empty() {
                continue;
            }
            // Check for root tag — try inline tags first, fall back to API
            let is_root = if let Some(tags) = e.get("tags").and_then(|v| v.as_array()) {
                has_tag_inline(tags, VFS_ROOT_TAG.0, VFS_ROOT_TAG.1)
            } else {
                let node_id = ensure_entity_id(id);
                let tags_val = self.backend.get_tags(&node_id).await?;
                has_tag(&tags_val, VFS_ROOT_TAG.0, VFS_ROOT_TAG.1)
            };
            if is_root {
                candidates.push(id.to_string());
            }
            // Fallback: detect root by name="/" prop (in case tags are stale).
            let name_prop = e.get("props").and_then(|p| p.get("name")).and_then(|v| v.as_str())
                .or_else(|| e.get("label").and_then(|v| v.as_str()));
            if name_prop == Some("/") && kind_match {
                fallback_candidates.push(id.to_string());
            }
        }

        if !candidates.is_empty() {
            candidates.sort();
            return Ok(candidates.into_iter().next().unwrap());
        }
        // Fallback: root entity exists but tag wasn't indexed. Re-tag it.
        if !fallback_candidates.is_empty() {
            fallback_candidates.sort();
            let id = fallback_candidates.into_iter().next().unwrap();
            let node_id = ensure_entity_id(&id);
            let _ = self.backend.add_tags(&node_id, vec![
                (VFS_ROOT_TAG.0.to_string(), VFS_ROOT_TAG.1.to_string()),
            ]).await;
            return Ok(id);
        }

        // Create the root directory.
        let resp = self
            .backend
            .add_entity(VFS_DIR_KIND, serde_json::json!({ "name": "/" }), None)
            .await?;
        let raw = resp
            .get("id")
            .and_then(|v| v.as_str())
            .context("add_entity did not return id")?;
        // Strip prefix if present (HTTP backend returns "entity:<hex>")
        let id = bare_hex(raw).to_string();
        let node_id = ensure_entity_id(&id);
        self.backend
            .add_tags(
                &node_id,
                vec![(VFS_ROOT_TAG.0.to_string(), VFS_ROOT_TAG.1.to_string())],
            )
            .await?;
        Ok(id)
    }

    // ── Path resolution ────────────────────────────────────────────

    fn split_path(path: &str) -> Result<Vec<&str>> {
        let path = path.trim();
        if !path.starts_with('/') {
            bail!("path must be absolute (start with /)");
        }
        Ok(path.split('/').filter(|s| !s.is_empty()).collect())
    }

    /// Resolve a path to `(node_id, edge_id)`. The edge_id is empty for root.
    pub async fn resolve(&self, path: &str) -> Result<Option<(String, String)>> {
        let components = Self::split_path(path)?;
        if components.is_empty() {
            let root = self.ensure_root().await?;
            return Ok(Some((ensure_entity_id(&root), String::new())));
        }
        let root = self.ensure_root().await?;
        let mut current = ensure_entity_id(&root);
        let mut last_edge_id = String::new();
        for component in &components {
            match self.find_child(&current, component).await? {
                Some((child_node, edge_id)) => {
                    current = child_node;
                    last_edge_id = edge_id;
                }
                None => return Ok(None),
            }
        }
        Ok(Some((current, last_edge_id)))
    }

    /// Find a child by name under a parent node. Returns `(child_node_id, edge_id)`.
    async fn find_child(&self, parent: &str, name: &str) -> Result<Option<(String, String)>> {
        let edges = self.backend.edges_of(parent).await?;
        let empty = vec![];
        let arr = edges.as_array().unwrap_or(&empty);
        let mut best: Option<(String, String)> = None;
        for edge in arr {
            // edges_of returns both directions; only process outgoing edges from parent.
            if edge.get("source").and_then(|v| v.as_str()) != Some(parent) {
                continue;
            }
            if edge.get("relation").and_then(|v| v.as_str()) != Some(VFS_CHILD_REL) {
                continue;
            }
            let edge_name = edge
                .get("props")
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if edge_name != name {
                continue;
            }
            let target = edge
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let eid = edge
                .get("edge_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            match &best {
                Some((_, existing_eid)) if *existing_eid <= eid => {}
                _ => best = Some((target, eid)),
            }
        }
        Ok(best)
    }

    // ── Mutations ──────────────────────────────────────────────────

    /// Create all intermediate directories. Returns the entity ID of the leaf.
    pub async fn mkdir(&self, path: &str) -> Result<String> {
        let components = Self::split_path(path)?;
        if components.is_empty() {
            return self.ensure_root().await;
        }
        let root = self.ensure_root().await?;
        let mut current = ensure_entity_id(&root);
        for component in &components {
            match self.find_child(&current, component).await? {
                Some((child, _)) => current = child,
                None => {
                    let new_dir = self.create_dir(component).await?;
                    let new_node = ensure_entity_id(&new_dir);
                    match self.create_child_edge(&current, &new_node, component).await {
                        Ok(_) => current = new_node,
                        Err(_) => {
                            // Race: another writer created this entry concurrently.
                            match self.find_child(&current, component).await? {
                                Some((existing, _)) => current = existing,
                                None => bail!("failed to create directory component '{component}'"),
                            }
                        }
                    }
                }
            }
        }
        Ok(bare_hex(&current).to_string())
    }

    async fn create_dir(&self, name: &str) -> Result<String> {
        let resp = self
            .backend
            .add_entity(VFS_DIR_KIND, serde_json::json!({ "name": name }), None)
            .await?;
        let raw = resp
            .get("id")
            .and_then(|v| v.as_str())
            .context("add_entity did not return id")?;
        // Strip prefix if present (HTTP backend returns "entity:<hex>")
        Ok(bare_hex(raw).to_string())
    }

    async fn create_child_edge(
        &self,
        parent: &str,
        child: &str,
        name: &str,
    ) -> Result<String> {
        if self.find_child(parent, name).await?.is_some() {
            bail!("entry '{name}' already exists in directory");
        }
        let mut props = BTreeMap::new();
        props.insert(
            "name".to_string(),
            serde_json::Value::String(name.to_string()),
        );
        let resp = self
            .backend
            .add_link(parent, child, VFS_CHILD_REL, None, props)
            .await?;
        resp.get("edge_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .context("add_link did not return edge_id")
    }

    /// Place a node at a VFS path. Creates intermediate directories as needed.
    pub async fn link(&self, path: &str, target: &str) -> Result<String> {
        let components = Self::split_path(path)?;
        if components.is_empty() {
            bail!("cannot link to root path");
        }
        let (parent_components, file_name) = components.split_at(components.len() - 1);
        let file_name = file_name[0];

        let root = self.ensure_root().await?;
        let mut current = ensure_entity_id(&root);
        for component in parent_components {
            match self.find_child(&current, component).await? {
                Some((child, _)) => current = child,
                None => {
                    let new_dir = self.create_dir(component).await?;
                    let new_node = ensure_entity_id(&new_dir);
                    match self.create_child_edge(&current, &new_node, component).await {
                        Ok(_) => current = new_node,
                        Err(_) => {
                            // Race: another writer created this entry concurrently.
                            match self.find_child(&current, component).await? {
                                Some((existing, _)) => current = existing,
                                None => bail!("failed to create directory component '{component}'"),
                            }
                        }
                    }
                }
            }
        }

        self.create_child_edge(&current, target, file_name).await
    }

    /// Remove an entry from a VFS path. Does not delete the underlying node.
    pub async fn unlink(&self, path: &str) -> Result<()> {
        let components = Self::split_path(path)?;
        if components.is_empty() {
            bail!("cannot unlink root");
        }
        let (parent_components, file_name) = components.split_at(components.len() - 1);
        let file_name = file_name[0];

        let root = self.ensure_root().await?;
        let mut current = ensure_entity_id(&root);
        for component in parent_components {
            match self.find_child(&current, component).await? {
                Some((child, _)) => current = child,
                None => bail!("path not found: component '{component}' does not exist"),
            }
        }

        match self.find_child(&current, file_name).await? {
            Some((_, edge_id)) => {
                self.backend.delete_link(&edge_id, &current).await?;
                Ok(())
            }
            None => bail!("'{file_name}' not found in directory"),
        }
    }

    /// Move/rename: unlink from old path, link at new path.
    pub async fn mv(&self, from: &str, to: &str) -> Result<()> {
        let (node_id, _) = self
            .resolve(from)
            .await?
            .context("source path not found")?;
        self.unlink(from).await?;
        self.link(to, &node_id).await?;
        Ok(())
    }

    // ── Queries ────────────────────────────────────────────────────

    /// List entries in a directory.
    pub async fn ls(&self, path: &str, recursive: bool) -> Result<Vec<VfsEntry>> {
        let (node_id, _) = self.resolve(path).await?.context("path not found")?;
        self.ls_node(&node_id, recursive).await
    }

    fn ls_node<'s>(
        &'s self,
        node_id: &'s str,
        recursive: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<VfsEntry>>> + Send + 's>>
    {
        Box::pin(async move {
            let edges = self.backend.edges_of(node_id).await?;
            let empty = vec![];
            let arr = edges.as_array().unwrap_or(&empty);
            // Deduplicate by name — keep smallest edge_id on conflict (CRDT tiebreaker).
            let mut seen: BTreeMap<String, (String, String)> = BTreeMap::new();
            for edge in arr {
                if edge.get("source").and_then(|v| v.as_str()) != Some(node_id) {
                    continue;
                }
                if edge.get("relation").and_then(|v| v.as_str()) != Some(VFS_CHILD_REL) {
                    continue;
                }
                let name = edge
                    .get("props")
                    .and_then(|p| p.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
                    .to_string();
                let target = edge
                    .get("target")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let edge_id = edge
                    .get("edge_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                match seen.get(&name) {
                    Some((_, existing_eid)) if *existing_eid <= edge_id => {}
                    _ => { seen.insert(name, (target, edge_id)); }
                }
            }
            let mut entries = Vec::new();
            for (name, (target, edge_id)) in &seen {
                let node_type = self.resolve_node_type(target).await;
                entries.push(VfsEntry {
                    name: name.clone(),
                    node_id: target.clone(),
                    node_type,
                    edge_id: edge_id.clone(),
                });
                if recursive && node_type == "dir" {
                    let sub = self.ls_node(target, true).await?;
                    for mut child in sub {
                        child.name = format!("{name}/{}", child.name);
                        entries.push(child);
                    }
                }
            }
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            Ok(entries)
        })
    }

    async fn resolve_node_type(&self, node_id: &str) -> &'static str {
        match node_type_prefix(node_id) {
            "entity" => {
                let hex_id = bare_hex(node_id);
                if let Ok(Some(entity)) = self.backend.get_entity(hex_id).await {
                    if entity.get("kind").and_then(|v| v.as_str()) == Some(VFS_DIR_KIND) {
                        return "dir";
                    }
                }
                "entity"
            }
            other => other,
        }
    }

    /// Render an ASCII tree from a path.
    pub async fn tree(&self, path: &str, max_depth: usize) -> Result<String> {
        let (node_id, _) = self.resolve(path).await?.context("path not found")?;
        let label = if path == "/" || path.is_empty() {
            "/".to_string()
        } else {
            path.rsplit('/').next().unwrap_or(path).to_string()
        };
        let mut buf = String::new();
        buf.push_str(&format!("{label}/\n"));
        self.tree_recurse(&node_id, &mut buf, "", max_depth, 0)
            .await?;
        Ok(buf)
    }

    fn tree_recurse<'s>(
        &'s self,
        node_id: &'s str,
        buf: &'s mut String,
        prefix: &'s str,
        max_depth: usize,
        depth: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 's>> {
        Box::pin(async move {
            if depth >= max_depth {
                return Ok(());
            }
            let entries = self.ls_node(node_id, false).await?;
            let count = entries.len();
            for (i, entry) in entries.iter().enumerate() {
                let is_last = i == count - 1;
                let connector = if is_last { "└── " } else { "├── " };
                let child_prefix_str = if is_last { "    " } else { "│   " };
                if entry.node_type == "dir" {
                    buf.push_str(&format!("{prefix}{connector}{}/\n", entry.name));
                    let next_prefix = format!("{prefix}{child_prefix_str}");
                    self.tree_recurse(&entry.node_id, buf, &next_prefix, max_depth, depth + 1)
                        .await?;
                } else {
                    buf.push_str(&format!(
                        "{prefix}{connector}{} [{}] ({})\n",
                        entry.name, entry.node_type, entry.node_id
                    ));
                }
            }
            Ok(())
        })
    }

    /// Find all VFS paths that lead to a given node.
    pub async fn find_paths(&self, target_node: &str) -> Result<Vec<String>> {
        let root = self.ensure_root().await?;
        let root_id = ensure_entity_id(&root);
        let mut paths = Vec::new();
        self.find_paths_recurse(&root_id, target_node, "", &mut paths, 20)
            .await?;
        paths.sort();
        Ok(paths)
    }

    fn find_paths_recurse<'s>(
        &'s self,
        current: &'s str,
        target: &'s str,
        prefix: &'s str,
        paths: &'s mut Vec<String>,
        max_depth: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 's>> {
        Box::pin(async move {
            if max_depth == 0 {
                return Ok(());
            }
            let edges = self.backend.edges_of(current).await?;
            let empty = vec![];
            let arr = edges.as_array().unwrap_or(&empty);
            for edge in arr {
                if edge.get("source").and_then(|v| v.as_str()) != Some(current) {
                    continue;
                }
                if edge.get("relation").and_then(|v| v.as_str()) != Some(VFS_CHILD_REL) {
                    continue;
                }
                let name = edge
                    .get("props")
                    .and_then(|p| p.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let child = edge
                    .get("target")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let child_path = format!("{prefix}/{name}");
                if child == target {
                    paths.push(child_path.clone());
                }
                if self.resolve_node_type(child).await == "dir" {
                    self.find_paths_recurse(child, target, &child_path, paths, max_depth - 1)
                        .await?;
                }
            }
            Ok(())
        })
    }
}

// ── Helpers ────────────────────────────────────────────────────────

fn node_type_prefix(node_id: &str) -> &'static str {
    if node_id.starts_with("entity:") {
        "entity"
    } else if node_id.starts_with("doc:") {
        "doc"
    } else if node_id.starts_with("attachment:") {
        "attachment"
    } else {
        "unknown"
    }
}

fn has_tag(tags_val: &serde_json::Value, scope: &str, label: &str) -> bool {
    if let Some(tags) = tags_val.get("tags").and_then(|v| v.as_array()) {
        return has_tag_inline(tags, scope, label);
    }
    false
}

/// Check tags from an inline array (as returned in list_all / list_entities responses).
fn has_tag_inline(tags: &[serde_json::Value], scope: &str, label: &str) -> bool {
    for tag in tags {
        let ts = tag.as_array().map(|a| {
            (
                a.first().and_then(|v| v.as_str()).unwrap_or_default(),
                a.get(1).and_then(|v| v.as_str()).unwrap_or_default(),
            )
        });
        if ts == Some((scope, label)) {
            return true;
        }
    }
    false
}

