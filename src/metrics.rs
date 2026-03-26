use prometheus::{Encoder, IntGaugeVec, Opts, Registry, TextEncoder};

pub struct Metrics {
    registry: Registry,
    pub service_healthy: IntGaugeVec,
    pub service_upgrade_pending: IntGaugeVec,
    pub service_busy: IntGaugeVec,
}

impl Metrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        let service_healthy = IntGaugeVec::new(
            Opts::new("mac_mgmt_service_healthy", "Whether the service is healthy (1) or not (0)"),
            &["service"],
        )
        .unwrap();

        let service_upgrade_pending = IntGaugeVec::new(
            Opts::new("mac_mgmt_service_upgrade_pending", "Whether an upgrade is pending (1) or not (0)"),
            &["service"],
        )
        .unwrap();

        let service_busy = IntGaugeVec::new(
            Opts::new("mac_mgmt_service_busy", "Whether the service is busy (1) or not (0)"),
            &["service"],
        )
        .unwrap();

        registry.register(Box::new(service_healthy.clone())).unwrap();
        registry.register(Box::new(service_upgrade_pending.clone())).unwrap();
        registry.register(Box::new(service_busy.clone())).unwrap();

        Metrics {
            registry,
            service_healthy,
            service_upgrade_pending,
            service_busy,
        }
    }

    pub fn render(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}
