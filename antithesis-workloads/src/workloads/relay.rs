//! Relay-facing workloads: access check, file/shell/log tunnels, TCP tunnel.

use anyhow::{Result, bail};
use async_trait::async_trait;

use super::{Outcome, Workload};
use crate::env::Ctx;
use crate::rng::Rng;

fn pick_prefix(ctx: &Ctx, rng: &mut Rng) -> Option<String> {
    rng.choose(&ctx.instances)
        .map(|id| ctx.env.relay_prefix(id))
}

/// The daemon is reachable through the relay.
pub struct RelayAccess;

#[async_trait]
impl Workload for RelayAccess {
    fn name(&self) -> &'static str {
        "relay-access"
    }
    async fn run(&self, ctx: &Ctx, rng: &mut Rng) -> Result<Outcome> {
        let Some(prefix) = pick_prefix(ctx, rng) else {
            return Ok(Outcome::Skipped("no instances"));
        };
        if !ctx.relay().is_online(&prefix).await {
            bail!("daemon {prefix} not online at relay");
        }
        Ok(Outcome::Done)
    }
}

/// List a file tunnel and assert it responds.
pub struct RelayFileTunnel;

#[async_trait]
impl Workload for RelayFileTunnel {
    fn name(&self) -> &'static str {
        "relay-file-tunnel"
    }
    async fn run(&self, ctx: &Ctx, rng: &mut Rng) -> Result<Outcome> {
        let Some(prefix) = pick_prefix(ctx, rng) else {
            return Ok(Outcome::Skipped("no instances"));
        };
        let _listing = ctx.relay().file_list(&prefix, "daemon-config").await?;
        Ok(Outcome::Done)
    }
}

/// Run a shell tunnel command and assert a clean exit.
pub struct RelayShellTunnel;

#[async_trait]
impl Workload for RelayShellTunnel {
    fn name(&self) -> &'static str {
        "relay-shell-tunnel"
    }
    async fn run(&self, ctx: &Ctx, rng: &mut Rng) -> Result<Outcome> {
        let Some(prefix) = pick_prefix(ctx, rng) else {
            return Ok(Outcome::Skipped("no instances"));
        };
        let out = ctx
            .relay()
            .shell_exec(&prefix, "daemon-status", None)
            .await?;
        // systemctl status exits non-zero for some states; just assert it ran.
        if out.exit_code.is_none() {
            bail!("daemon-status produced no exit code");
        }
        Ok(Outcome::Done)
    }
}

/// Fetch logs through the relay.
pub struct RelayLogs;

#[async_trait]
impl Workload for RelayLogs {
    fn name(&self) -> &'static str {
        "relay-logs"
    }
    async fn run(&self, ctx: &Ctx, rng: &mut Rng) -> Result<Outcome> {
        let Some(prefix) = pick_prefix(ctx, rng) else {
            return Ok(Outcome::Skipped("no instances"));
        };
        let _logs = ctx.relay().logs(&prefix, Some(50)).await?;
        Ok(Outcome::Done)
    }
}

/// List TCP tunnels advertised by the daemon.
pub struct RelayTcpTunnel;

#[async_trait]
impl Workload for RelayTcpTunnel {
    fn name(&self) -> &'static str {
        "relay-tcp-tunnel"
    }
    async fn run(&self, ctx: &Ctx, rng: &mut Rng) -> Result<Outcome> {
        let Some(prefix) = pick_prefix(ctx, rng) else {
            return Ok(Outcome::Skipped("no instances"));
        };
        let _tunnels = ctx.relay().tunnels(&prefix).await?;
        Ok(Outcome::Done)
    }
}
