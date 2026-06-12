use std::fmt;

#[derive(Debug, Clone)]
pub enum DaemonEvent {
    DaemonStarted,
    DaemonStopped,
    ServiceCrashed {
        service: String,
        exit_code: Option<i32>,
    },
    ServiceUnhealthy {
        service: String,
    },
    ServiceRecovered {
        service: String,
    },
    UpgradeInstalled {
        service: String,
    },
    UpgradeFailed {
        service: String,
        error: String,
    },
    BackupCompleted {
        snapshot_id: String,
        duration_secs: u64,
    },
    BackupFailed {
        error: String,
    },
}

impl DaemonEvent {
    pub fn kind(&self) -> &str {
        match self {
            Self::DaemonStarted => "daemon_started",
            Self::DaemonStopped => "daemon_stopped",
            Self::ServiceCrashed { .. } => "service_crashed",
            Self::ServiceUnhealthy { .. } => "service_unhealthy",
            Self::ServiceRecovered { .. } => "service_recovered",
            Self::UpgradeInstalled { .. } => "upgrade_installed",
            Self::UpgradeFailed { .. } => "upgrade_failed",
            Self::BackupCompleted { .. } => "backup_completed",
            Self::BackupFailed { .. } => "backup_failed",
        }
    }
}

impl fmt::Display for DaemonEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DaemonStarted => write!(f, "mac-mgmt daemon started"),
            Self::DaemonStopped => write!(f, "mac-mgmt daemon stopped"),
            Self::ServiceCrashed { service, exit_code } => {
                write!(f, "{service} crashed")?;
                if let Some(code) = exit_code {
                    write!(f, " (exit code {code})")?;
                }
                Ok(())
            }
            Self::ServiceUnhealthy { service } => {
                write!(f, "{service} health check failed")
            }
            Self::ServiceRecovered { service } => {
                write!(f, "{service} recovered and is healthy again")
            }
            Self::UpgradeInstalled { service } => {
                write!(f, "{service} upgrade installed successfully")
            }
            Self::UpgradeFailed { service, error } => {
                write!(f, "{service} upgrade failed: {error}")
            }
            Self::BackupCompleted {
                snapshot_id,
                duration_secs,
            } => {
                write!(
                    f,
                    "backup completed (snapshot {snapshot_id}, {duration_secs}s)"
                )
            }
            Self::BackupFailed { error } => {
                write!(f, "backup failed: {error}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_kind_matches() {
        assert_eq!(DaemonEvent::DaemonStarted.kind(), "daemon_started");
        assert_eq!(DaemonEvent::DaemonStopped.kind(), "daemon_stopped");
        assert_eq!(
            DaemonEvent::ServiceCrashed {
                service: "test".into(),
                exit_code: Some(1),
            }
            .kind(),
            "service_crashed"
        );
        assert_eq!(
            DaemonEvent::ServiceUnhealthy {
                service: "test".into(),
            }
            .kind(),
            "service_unhealthy"
        );
        assert_eq!(
            DaemonEvent::ServiceRecovered {
                service: "test".into(),
            }
            .kind(),
            "service_recovered"
        );
        assert_eq!(
            DaemonEvent::UpgradeInstalled {
                service: "test".into(),
            }
            .kind(),
            "upgrade_installed"
        );
        assert_eq!(
            DaemonEvent::UpgradeFailed {
                service: "test".into(),
                error: "oops".into(),
            }
            .kind(),
            "upgrade_failed"
        );
    }

    #[test]
    fn event_display_messages() {
        assert_eq!(
            DaemonEvent::DaemonStarted.to_string(),
            "mac-mgmt daemon started"
        );
        assert_eq!(
            DaemonEvent::DaemonStopped.to_string(),
            "mac-mgmt daemon stopped"
        );
        assert_eq!(
            DaemonEvent::ServiceCrashed {
                service: "ollama".into(),
                exit_code: Some(1),
            }
            .to_string(),
            "ollama crashed (exit code 1)"
        );
        assert_eq!(
            DaemonEvent::ServiceCrashed {
                service: "ollama".into(),
                exit_code: None,
            }
            .to_string(),
            "ollama crashed"
        );
        assert_eq!(
            DaemonEvent::ServiceRecovered {
                service: "openclaw".into(),
            }
            .to_string(),
            "openclaw recovered and is healthy again"
        );
    }
}
