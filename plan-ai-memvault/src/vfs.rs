//! MCP-side VFS adapter — thin wrapper over `memvault_api::vfs` that adds
//! the higher-level operations the MCP tools expose (ls/tree/unlink/mv/find).

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use memvault_api::MemvaultClient;
use memvault_api::vfs as api_vfs;
use memvault_core::{BucketId, EdgeId, EntityId, NodeRef};

/// A single entry in a directory listing.
#[derive(Debug, Serialize)]
pub struct VfsEntry {
    pub name: String,
    pub node_id: String,
    pub node_type: String,
    pub edge_id: String,
}

/// The VFS handle for the MCP. Pins the operative bucket (agent bucket by default).
pub struct Vfs {
    client: Arc<dyn MemvaultClient>,
    bucket: BucketId,
}

impl Vfs {
    /// Construct a VFS handle, falling back to the daemon's default bucket
    /// when no explicit bucket is provided.
    pub async fn new(client: Arc<dyn MemvaultClient>, bucket: Option<BucketId>) -> Self {
        let bucket = match bucket {
            Some(b) => b,
            None => api_vfs::default_bucket(&*client).await,
        };
        Self { client, bucket }
    }

    fn split_path(path: &str) -> Result<Vec<&str>> {
        let path = path.trim();
        if !path.starts_with('/') {
            bail!("path must be absolute (start with /)");
        }
        Ok(path.split('/').filter(|s| !s.is_empty()).collect())
    }

    pub async fn resolve(&self, path: &str) -> Result<Option<(NodeRef, Option<EdgeId>)>> {
        api_vfs::resolve_path(&*self.client, &self.bucket, path)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    pub async fn mkdir(&self, path: &str) -> Result<EntityId> {
        let leaf = api_vfs::ensure_dir_path(&*self.client, &self.bucket, path)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        match leaf {
            NodeRef::Entity(id) => Ok(id),
            _ => bail!("path resolved to non-entity"),
        }
    }

    pub async fn link(&self, path: &str, target: &str) -> Result<EdgeId> {
        let target_ref = NodeRef::from_tag_label(target)
            .ok_or_else(|| anyhow::anyhow!("invalid target: {target}"))?;
        api_vfs::link_at_path(&*self.client, &self.bucket, path, &target_ref)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    pub async fn unlink(&self, path: &str) -> Result<()> {
        let components = Self::split_path(path)?;
        if components.is_empty() {
            bail!("cannot unlink root");
        }
        let (parent_parts, file_name) = components.split_at(components.len() - 1);
        let file_name = file_name[0];
        let parent_path = if parent_parts.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", parent_parts.join("/"))
        };
        let (parent, _) = self
            .resolve(&parent_path)
            .await?
            .with_context(|| format!("parent not found: {parent_path}"))?;
        match api_vfs::find_named_child(&*self.client, &parent, file_name)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?
        {
            Some((_, edge_id)) => {
                self.client
                    .remove_link_from(&parent, &edge_id)
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                Ok(())
            }
            None => bail!("'{file_name}' not found in directory"),
        }
    }

    pub async fn mv(&self, from: &str, to: &str) -> Result<()> {
        let (node, _) = self.resolve(from).await?.context("source path not found")?;
        let target_label = node.tag_label();
        self.unlink(from).await?;
        self.link(to, &target_label).await?;
        Ok(())
    }

    pub async fn ls(&self, path: &str, recursive: bool) -> Result<Vec<VfsEntry>> {
        let (node, _) = self.resolve(path).await?.context("path not found")?;
        self.ls_node(&node, recursive).await
    }

    fn ls_node<'s>(
        &'s self,
        node: &'s NodeRef,
        recursive: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<VfsEntry>>> + Send + 's>>
    {
        Box::pin(async move {
            let children = api_vfs::list_children(&*self.client, node)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let mut entries = Vec::new();
            for (name, target, edge_id) in &children {
                let node_type = api_vfs::resolve_node_type(&*self.client, target).await;
                entries.push(VfsEntry {
                    name: name.clone(),
                    node_id: target.tag_label(),
                    node_type: node_type.clone(),
                    edge_id: hex::encode(edge_id.0),
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

    pub async fn tree(&self, path: &str, max_depth: usize) -> Result<String> {
        let (node, _) = self.resolve(path).await?.context("path not found")?;
        let label = if path == "/" || path.is_empty() {
            "/".to_string()
        } else {
            path.rsplit('/').next().unwrap_or(path).to_string()
        };
        let mut buf = String::new();
        buf.push_str(&format!("{label}/\n"));
        self.tree_recurse(&node, &mut buf, "", max_depth, 0).await?;
        Ok(buf)
    }

    fn tree_recurse<'s>(
        &'s self,
        node: &'s NodeRef,
        buf: &'s mut String,
        prefix: &'s str,
        max_depth: usize,
        depth: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 's>> {
        Box::pin(async move {
            if depth >= max_depth {
                return Ok(());
            }
            let entries = self.ls_node(node, false).await?;
            let count = entries.len();
            for (i, entry) in entries.iter().enumerate() {
                let is_last = i == count - 1;
                let connector = if is_last { "└── " } else { "├── " };
                let child_prefix_str = if is_last { "    " } else { "│   " };
                if entry.node_type == "dir" {
                    buf.push_str(&format!("{prefix}{connector}{}/\n", entry.name));
                    let next_prefix = format!("{prefix}{child_prefix_str}");
                    let child_ref = NodeRef::from_tag_label(&entry.node_id)
                        .context("invalid child node id")?;
                    self.tree_recurse(&child_ref, buf, &next_prefix, max_depth, depth + 1)
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
    pub async fn find_paths(&self, target: &str) -> Result<Vec<String>> {
        let target_ref = NodeRef::from_tag_label(target)
            .ok_or_else(|| anyhow::anyhow!("invalid target: {target}"))?;
        let (root, _) = self
            .resolve("/")
            .await?
            .context("root not found")?;
        let mut paths = Vec::new();
        self.find_paths_recurse(&root, &target_ref, "", &mut paths, 20)
            .await?;
        paths.sort();
        Ok(paths)
    }

    fn find_paths_recurse<'s>(
        &'s self,
        current: &'s NodeRef,
        target: &'s NodeRef,
        prefix: &'s str,
        paths: &'s mut Vec<String>,
        max_depth: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 's>> {
        Box::pin(async move {
            if max_depth == 0 {
                return Ok(());
            }
            let children = api_vfs::list_children(&*self.client, current)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            for (name, child, _) in &children {
                let child_path = format!("{prefix}/{name}");
                if child == target {
                    paths.push(child_path.clone());
                }
                if api_vfs::resolve_node_type(&*self.client, child).await == "dir" {
                    self.find_paths_recurse(child, target, &child_path, paths, max_depth - 1)
                        .await?;
                }
            }
            Ok(())
        })
    }
}

// Used by ls/list serialization — keeps BTreeMap usage out of unused warnings
// when this module is compiled standalone.
#[allow(dead_code)]
fn _props_compat() -> BTreeMap<String, serde_json::Value> {
    BTreeMap::new()
}
