//! Security posture collection: system-level findings expressed as
//! [`SecurityFinding`] structs. macOS checks SIP/FileVault/firewall/
//! Gatekeeper/XProtect; Linux checks SELinux/AppArmor/ufw/nftables/FDE.
//!
//! All probes swallow their own errors — an unsupported tool or a missing
//! binary simply omits that finding. We never block the assessment on a
//! single failing posture check.

use std::process::Command;

use anyhow::Result;

use mac_mgmt_common::{FindingSeverity, SecurityFinding};

pub async fn collect() -> Result<Vec<SecurityFinding>> {
    tokio::task::spawn_blocking(|| Ok(collect_blocking())).await?
}

fn collect_blocking() -> Vec<SecurityFinding> {
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
        Vec::new()
    }
}

// ── Helper to build a finding from an Option<bool> check ──

fn bool_finding(
    id: &str,
    severity: FindingSeverity,
    msg_pass: &str,
    msg_fail: &str,
    value: Option<bool>,
) -> Option<SecurityFinding> {
    let pass = value?;
    Some(SecurityFinding {
        id: id.to_string(),
        severity,
        message: if pass { msg_pass } else { msg_fail }.to_string(),
        pass,
    })
}

// ── macOS ──

#[cfg(target_os = "macos")]
fn collect_macos() -> Vec<SecurityFinding> {
    let mut findings = Vec::new();

    if let Some(f) = bool_finding(
        "macos_sip",
        FindingSeverity::High,
        "System Integrity Protection enabled",
        "System Integrity Protection disabled",
        csrutil_enabled(),
    ) {
        findings.push(f);
    }

    if let Some(f) = bool_finding(
        "macos_filevault",
        FindingSeverity::High,
        "FileVault encryption enabled",
        "FileVault encryption disabled",
        fdesetup_enabled(),
    ) {
        findings.push(f);
    }

    if let Some(f) = bool_finding(
        "macos_firewall",
        FindingSeverity::Medium,
        "Application Firewall enabled",
        "Application Firewall disabled",
        alf_enabled(),
    ) {
        findings.push(f);
    }

    if let Some(f) = bool_finding(
        "macos_gatekeeper",
        FindingSeverity::Medium,
        "Gatekeeper assessments enabled",
        "Gatekeeper assessments disabled",
        spctl_enabled(),
    ) {
        findings.push(f);
    }

    if let Some(version) = xprotect_version() {
        findings.push(SecurityFinding {
            id: "macos_xprotect".to_string(),
            severity: FindingSeverity::Info,
            message: format!("XProtect definitions version {version}"),
            pass: true,
        });
    }

    findings
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

// ── Linux ──

#[cfg(target_os = "linux")]
fn collect_linux() -> Vec<SecurityFinding> {
    let mut findings = Vec::new();

    if let Some(mode) = getenforce_mode() {
        let pass = mode == "enforcing";
        findings.push(SecurityFinding {
            id: "linux_selinux".to_string(),
            severity: FindingSeverity::Medium,
            message: format!("SELinux mode: {mode}"),
            pass,
        });
    }

    if let Some(count) = apparmor_profile_count() {
        findings.push(SecurityFinding {
            id: "linux_apparmor".to_string(),
            severity: FindingSeverity::Info,
            message: format!("{count} AppArmor profiles loaded"),
            pass: count > 0,
        });
    }

    if let Some(f) = bool_finding(
        "linux_ufw",
        FindingSeverity::Medium,
        "ufw firewall active",
        "ufw firewall inactive",
        ufw_active(),
    ) {
        findings.push(f);
    }

    if let Some(count) = nftables_rule_count() {
        let pass = count > 0;
        findings.push(SecurityFinding {
            id: "linux_nftables".to_string(),
            severity: if pass {
                FindingSeverity::Info
            } else {
                FindingSeverity::Medium
            },
            message: if pass {
                format!("{count} nftables rules loaded")
            } else {
                "nftables installed but no rules loaded".to_string()
            },
            pass,
        });
    }

    if let Some(f) = bool_finding(
        "linux_fde",
        FindingSeverity::High,
        "Full-disk encryption detected on root",
        "No full-disk encryption on root",
        luks_present_on_root(),
    ) {
        findings.push(f);
    }

    findings
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
    let s = std::fs::read_to_string("/sys/kernel/security/apparmor/profiles").ok()?;
    Some(s.lines().count() as u32)
}

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
