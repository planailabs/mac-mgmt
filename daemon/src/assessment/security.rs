//! Security posture: SIP/FileVault/firewall/Gatekeeper/XProtect on macOS;
//! SELinux/AppArmor/ufw/nftables/FDE on Linux. Booleans + version strings only.
//!
//! All probes swallow their own errors — an unsupported tool or a missing
//! binary leaves the corresponding field as `None`. We never block the
//! assessment on a single failing posture check.

use std::process::Command;

use anyhow::Result;

use mac_mgmt_common::SecurityPosture;

pub async fn collect() -> Result<SecurityPosture> {
    tokio::task::spawn_blocking(|| Ok(collect_blocking())).await?
}

fn collect_blocking() -> SecurityPosture {
    #[cfg(target_os = "macos")]
    {
        collect_macos()
    }
    #[cfg(target_os = "linux")]
    {
        collect_linux()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        SecurityPosture::default()
    }
}

#[cfg(target_os = "macos")]
fn collect_macos() -> SecurityPosture {
    SecurityPosture {
        sip_enabled: csrutil_enabled(),
        filevault_enabled: fdesetup_enabled(),
        firewall_enabled: alf_enabled(),
        gatekeeper_enabled: spctl_enabled(),
        xprotect_version: xprotect_version(),
        ..Default::default()
    }
}

#[cfg(target_os = "macos")]
fn csrutil_enabled() -> Option<bool> {
    let out = Command::new("csrutil").arg("status").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
    if s.contains("enabled") {
        Some(true)
    } else if s.contains("disabled") {
        Some(false)
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn fdesetup_enabled() -> Option<bool> {
    let out = Command::new("fdesetup").arg("status").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
    if s.contains("filevault is on") {
        Some(true)
    } else if s.contains("filevault is off") {
        Some(false)
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn alf_enabled() -> Option<bool> {
    let out = Command::new("defaults")
        .args(["read", "/Library/Preferences/com.apple.alf", "globalstate"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let state: u32 = s.parse().ok()?;
    Some(state > 0)
}

#[cfg(target_os = "macos")]
fn spctl_enabled() -> Option<bool> {
    let out = Command::new("spctl").arg("--status").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
    Some(s.contains("assessments enabled"))
}

#[cfg(target_os = "macos")]
fn xprotect_version() -> Option<String> {
    let out = Command::new("defaults")
        .args([
            "read",
            "/Library/Apple/System/Library/CoreServices/XProtect.bundle/Contents/Resources/XProtect.meta",
            "Version",
        ])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

#[cfg(target_os = "linux")]
fn collect_linux() -> SecurityPosture {
    SecurityPosture {
        selinux_mode: getenforce_mode(),
        apparmor_profiles: apparmor_profile_count(),
        ufw_active: ufw_active(),
        nftables_rule_count: nftables_rule_count(),
        fde_enabled: luks_present_on_root(),
        ..Default::default()
    }
}

#[cfg(target_os = "linux")]
fn getenforce_mode() -> Option<String> {
    let out = Command::new("getenforce").output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_lowercase())
}

#[cfg(target_os = "linux")]
fn apparmor_profile_count() -> Option<u32> {
    // /sys/kernel/security/apparmor/profiles lists loaded profiles; one per line.
    let s = std::fs::read_to_string("/sys/kernel/security/apparmor/profiles").ok()?;
    Some(s.lines().count() as u32)
}

/// `ufw status` reports "Status: active" or "Status: inactive". The binary
/// also exits 0 in both cases, so success + absence of either string means
/// an unexpected ufw version — return `None` rather than guessing.
#[cfg(target_os = "linux")]
fn ufw_active() -> Option<bool> {
    let out = Command::new("ufw").arg("status").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
    if s.contains("status: active") {
        Some(true)
    } else if s.contains("status: inactive") {
        Some(false)
    } else {
        None
    }
}

/// Parse `nft --json list ruleset` and count top-level `"rule"` entries.
///
/// The document shape is:
/// ```json
/// { "nftables": [ {"metainfo": ...}, {"table": ...}, {"chain": ...},
///                 {"rule": ...}, {"rule": ...} ] }
/// ```
///
/// `nft` needs CAP_NET_ADMIN (effectively root) to read the netlink rules,
/// which our daemon has when running under the system launchd/systemd unit.
/// A missing binary, permission denial, or a JSON-parse failure all return
/// `None` so callers can distinguish "absent" from `Some(0)` = "installed
/// but no rules configured" (itself a posture signal worth surfacing).
#[cfg(target_os = "linux")]
fn nftables_rule_count() -> Option<u32> {
    let out = Command::new("nft")
        .args(["--json", "list", "ruleset"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let arr = v.get("nftables")?.as_array()?;
    let count = arr
        .iter()
        .filter(|entry| {
            entry
                .as_object()
                .is_some_and(|obj| obj.contains_key("rule"))
        })
        .count();
    Some(count as u32)
}

#[cfg(target_os = "linux")]
fn luks_present_on_root() -> Option<bool> {
    let out = Command::new("lsblk")
        .args(["-o", "TYPE,MOUNTPOINT", "-n"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    for line in s.lines() {
        let mut parts = line.split_whitespace();
        let (Some(ty), Some(mp)) = (parts.next(), parts.next()) else {
            continue;
        };
        if mp == "/" {
            return Some(ty == "crypt");
        }
    }
    None
}
