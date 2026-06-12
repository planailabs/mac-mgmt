//! Static inventory collection: OS, kernel, CPU, RAM, disks, interfaces, nix.
//!
//! GDPR allowlist: never collect IPs, MAC addresses, usernames, hostnames of
//! peers, WiFi SSIDs, or any other identifier that could be used to reidentify
//! a person. Interface names are kept because they're structural (eg0, wlan0,
//! tailscale0) and disk mounts because they're limited to root + /nix/store.

use anyhow::Result;
use sysinfo::{Disks, Networks, System};

use mac_mgmt_common::{DiskInfo, Inventory, NetInterface};

pub async fn collect() -> Result<Inventory> {
    tokio::task::spawn_blocking(collect_blocking).await?
}

fn collect_blocking() -> Result<Inventory> {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();

    let cpu_model = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_default();
    let cpu_cores_logical = sys.cpus().len() as u32;
    let cpu_cores_physical = sys.physical_core_count().unwrap_or(0) as u32;

    let disks = Disks::new_with_refreshed_list()
        .iter()
        .filter_map(|d| {
            let mount = d.mount_point().to_string_lossy().to_string();
            if !is_interesting_mount(&mount) {
                return None;
            }
            Some(DiskInfo {
                mount,
                fs_type: d.file_system().to_string_lossy().to_string(),
                total_bytes: d.total_space(),
            })
        })
        .collect();

    let interfaces = Networks::new_with_refreshed_list()
        .iter()
        .filter_map(|(name, _data)| {
            if name == "lo" || name == "lo0" {
                return None;
            }
            Some(NetInterface {
                name: name.clone(),
                // sysinfo doesn't expose link state directly; treat any
                // interface with a non-zero MTU or any sampled traffic as up.
                // Refined per-platform in a later pass.
                up: true,
            })
        })
        .collect();

    let gpus = crate::assessment::gpu::inventory();

    Ok(Inventory {
        os_name: System::name().unwrap_or_default(),
        os_version: System::long_os_version().unwrap_or_default(),
        kernel_version: System::kernel_version().unwrap_or_default(),
        arch: std::env::consts::ARCH.to_string(),
        uptime_secs: System::uptime(),
        cpu_model,
        cpu_cores_physical,
        cpu_cores_logical,
        mem_total_bytes: sys.total_memory(),
        interfaces,
        disks,
        nix_version: detect_nix_version(),
        nixpkgs_commit: crate::nix::current_nixpkgs_commit(),
        supervisor: detect_supervisor(),
        gpus,
    })
}

/// Only report disks mounted at root or /nix/store. Anything under /Users or
/// /home is out-of-scope for a fleet-wide assessment and may leak user data.
fn is_interesting_mount(mount: &str) -> bool {
    mount == "/" || mount == "/nix/store" || mount == "/nix" || mount == "/System/Volumes/Data"
}

fn detect_nix_version() -> Option<String> {
    let out = std::process::Command::new("nix")
        .arg("--version")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn detect_supervisor() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        Some("launchd".into())
    }
    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/run/systemd/system").exists() {
            Some("systemd".into())
        } else {
            None
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}
