use prometheus::{Encoder, IntGaugeVec, Opts, Registry, TextEncoder};

pub struct Metrics {
    registry: Registry,
    pub service_healthy: IntGaugeVec,
    pub service_upgrade_pending: IntGaugeVec,
    pub service_busy: IntGaugeVec,
    pub daemon_version: String,
    pub started_at: std::time::Instant,
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
            daemon_version: env!("CARGO_PKG_VERSION").to_string(),
            started_at: std::time::Instant::now(),
        }
    }

    pub fn register_collector(&self, collector: Box<dyn prometheus::core::Collector>) -> prometheus::Result<()> {
        self.registry.register(collector)
    }

    pub fn status(&self) -> (String, u64, Vec<(String, bool, bool, bool)>) {
        let uptime = self.started_at.elapsed().as_secs();
        let families = self.registry.gather();
        let mut service_names: Vec<String> = Vec::new();

        for family in &families {
            if family.name() == "mac_mgmt_service_healthy" {
                for metric in family.get_metric() {
                    for label in metric.get_label() {
                        if label.name() == "service" {
                            service_names.push(label.value().to_string());
                        }
                    }
                }
            }
        }

        let services = service_names
            .iter()
            .map(|name| {
                (
                    name.clone(),
                    self.service_healthy.with_label_values(&[name]).get() == 1,
                    self.service_upgrade_pending
                        .with_label_values(&[name])
                        .get()
                        == 1,
                    self.service_busy.with_label_values(&[name]).get() == 1,
                )
            })
            .collect();

        (self.daemon_version.clone(), uptime, services)
    }

    pub fn render(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}
