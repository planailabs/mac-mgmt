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
        linux_firewall: linux_firewall_status(),
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
    Some(
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .to_lowercase(),
    )
}

#[cfg(target_os = "linux")]
fn apparmor_profile_count() -> Option<u32> {
    // /sys/kernel/security/apparmor/profiles lists loaded profiles; one per line.
    let s = std::fs::read_to_string("/sys/kernel/security/apparmor/profiles").ok()?;
    Some(s.lines().count() as u32)
}

#[cfg(target_os = "linux")]
fn linux_firewall_status() -> Option<String> {
    // Prefer ufw if present; fall back to a simple nftables rule count.
    if let Ok(out) = Command::new("ufw").arg("status").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
            if s.contains("status: active") {
                return Some("ufw:active".into());
            } else if s.contains("status: inactive") {
                return Some("ufw:inactive".into());
            }
        }
    }
    if let Ok(out) = Command::new("nft").args(["list", "ruleset"]).output() {
        if out.status.success() {
            let rules = String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|l| l.trim_start().starts_with("ip") || l.contains("chain"))
                .count();
            return Some(format!("nft:{rules}"));
        }
    }
    None
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
        let (Some(ty), Some(mp)) = (parts.next(), parts.next()) else { continue };
        if mp == "/" {
            return Some(ty == "crypt");
        }
    }
    None
}
