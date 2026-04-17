//! Helpers for the federated `/metrics` endpoint.
//!
//! Each per-daemon Prometheus exposition is parsed with `prometheus-parse`,
//! the federation labels (`instance_id`, `hostname`, `cluster_id`) are
//! injected into every sample, and the result is converted into typed
//! `prometheus::proto::MetricFamily` values that the official
//! `prometheus::TextEncoder` can render. The federation handler itself lives
//! in `ws_handler.rs` (next to the existing `proxy_metrics`) and only calls
//! into this module.
//!
//! Counter / Gauge / Histogram / Summary samples are preserved as their
//! original type. The `prometheus-parse` crate splits histogram and summary
//! lines so the bucket / quantile values arrive as `Value::Histogram` /
//! `Value::Summary`, while the matching `_sum` / `_count` lines fall through
//! as `Value::Untyped`. We re-fuse them into a single typed `Histogram` /
//! `Summary` `Metric` by matching on the base name + the rest of the labels.

use std::collections::{BTreeMap, HashMap, HashSet};

use prometheus::Encoder;
use prometheus::proto::{
    Bucket, Counter, Gauge, Histogram, LabelPair, Metric, MetricFamily, MetricType, Quantile,
    Summary, Untyped,
};
use prometheus_parse::{Sample, Scrape, Value};
use protobuf::MessageField;

/// Standard Prometheus 0.0.4 text content type.
pub const PROMETHEUS_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

/// Parse a single daemon's `/metrics` body, inject `instance_id`, `hostname`,
/// `cluster_id` and `cluster_name` labels into every sample, and produce
/// typed families.
pub fn parse_and_relabel(
    body: &str,
    instance_id: &str,
    hostname: &str,
    cluster_id: &str,
    cluster_name: &str,
) -> Result<Vec<MetricFamily>, std::io::Error> {
    let scrape = Scrape::parse(body.lines().map(|l| Ok(l.to_string())))?;

    // First pass: identify which base names are histograms / summaries so we
    // know how to route their `_sum` / `_count` lines, which the parser
    // returns as `Value::Untyped`.
    let mut hist_bases: HashSet<String> = HashSet::new();
    let mut summary_bases: HashSet<String> = HashSet::new();
    for s in &scrape.samples {
        match &s.value {
            Value::Histogram(_) => {
                hist_bases.insert(s.metric.clone());
            }
            Value::Summary(_) => {
                summary_bases.insert(s.metric.clone());
            }
            _ => {}
        }
    }

    let mut counters: BTreeMap<String, MetricFamily> = BTreeMap::new();
    let mut gauges: BTreeMap<String, MetricFamily> = BTreeMap::new();
    let mut untypeds: BTreeMap<String, MetricFamily> = BTreeMap::new();
    let mut histograms: HashMap<(String, String), HistogramAcc> = HashMap::new();
    let mut summaries: HashMap<(String, String), SummaryAcc> = HashMap::new();

    let federation = [
        ("instance_id", instance_id),
        ("hostname", hostname),
        ("cluster_id", cluster_id),
        ("cluster_name", cluster_name),
    ];

    for sample in scrape.samples {
        let base_labels = build_labels(&sample, &federation);

        match sample.value {
            Value::Counter(v) => {
                push_counter(&mut counters, &sample.metric, &base_labels, v);
            }
            Value::Gauge(v) => {
                push_gauge(&mut gauges, &sample.metric, &base_labels, v);
            }
            Value::Untyped(v) => {
                if route_sum_or_count(
                    &sample.metric,
                    &base_labels,
                    v,
                    &hist_bases,
                    &summary_bases,
                    &mut histograms,
                    &mut summaries,
                ) {
                    continue;
                }
                push_untyped(&mut untypeds, &sample.metric, &base_labels, v);
            }
            Value::Histogram(buckets) => {
                let key = labels_key(&base_labels);
                let acc = histograms
                    .entry((sample.metric.clone(), key))
                    .or_insert_with(|| HistogramAcc::new(base_labels.clone()));
                for hc in buckets {
                    acc.buckets.push((hc.less_than, hc.count));
                }
            }
            Value::Summary(quantiles) => {
                let key = labels_key(&base_labels);
                let acc = summaries
                    .entry((sample.metric.clone(), key))
                    .or_insert_with(|| SummaryAcc::new(base_labels.clone()));
                for sc in quantiles {
                    acc.quantiles.push((sc.quantile, sc.count));
                }
            }
        }
    }

    let mut histogram_families: BTreeMap<String, MetricFamily> = BTreeMap::new();
    for ((base_name, _key), acc) in histograms {
        let family = histogram_families
            .entry(base_name.clone())
            .or_insert_with(|| new_family(&base_name, MetricType::HISTOGRAM));
        family.mut_metric().push(acc.into_metric());
    }

    let mut summary_families: BTreeMap<String, MetricFamily> = BTreeMap::new();
    for ((base_name, _key), acc) in summaries {
        let family = summary_families
            .entry(base_name.clone())
            .or_insert_with(|| new_family(&base_name, MetricType::SUMMARY));
        family.mut_metric().push(acc.into_metric());
    }

    let mut all = Vec::new();
    all.extend(counters.into_values());
    all.extend(gauges.into_values());
    all.extend(untypeds.into_values());
    all.extend(histogram_families.into_values());
    all.extend(summary_families.into_values());
    Ok(all)
}

/// Encode families to the Prometheus 0.0.4 text exposition format.
pub fn encode_families(families: &[MetricFamily]) -> Result<Vec<u8>, prometheus::Error> {
    let encoder = prometheus::TextEncoder::new();
    let mut buf = Vec::new();
    encoder.encode(families, &mut buf)?;
    Ok(buf)
}

/// Append (or overwrite) `key=value` in a label list, preserving insertion
/// order for any other entries. Federation labels always win on collision.
pub fn upsert(labels: &mut Vec<(String, String)>, key: &str, value: &str) {
    if let Some(slot) = labels.iter_mut().find(|(k, _)| k == key) {
        slot.1 = value.to_string();
    } else {
        labels.push((key.to_string(), value.to_string()));
    }
}

fn build_labels(sample: &Sample, federation: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = sample
        .labels
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (k, v) in federation {
        upsert(&mut out, k, v);
    }
    out
}

fn route_sum_or_count(
    metric_name: &str,
    base_labels: &[(String, String)],
    value: f64,
    hist_bases: &HashSet<String>,
    summary_bases: &HashSet<String>,
    histograms: &mut HashMap<(String, String), HistogramAcc>,
    summaries: &mut HashMap<(String, String), SummaryAcc>,
) -> bool {
    if let Some(base) = metric_name.strip_suffix("_sum") {
        if hist_bases.contains(base) {
            let key = labels_key(base_labels);
            let acc = histograms
                .entry((base.to_string(), key))
                .or_insert_with(|| HistogramAcc::new(base_labels.to_vec()));
            acc.sample_sum = Some(value);
            return true;
        }
        if summary_bases.contains(base) {
            let key = labels_key(base_labels);
            let acc = summaries
                .entry((base.to_string(), key))
                .or_insert_with(|| SummaryAcc::new(base_labels.to_vec()));
            acc.sample_sum = Some(value);
            return true;
        }
    }
    if let Some(base) = metric_name.strip_suffix("_count") {
        if hist_bases.contains(base) {
            let key = labels_key(base_labels);
            let acc = histograms
                .entry((base.to_string(), key))
                .or_insert_with(|| HistogramAcc::new(base_labels.to_vec()));
            acc.sample_count = Some(value as u64);
            return true;
        }
        if summary_bases.contains(base) {
            let key = labels_key(base_labels);
            let acc = summaries
                .entry((base.to_string(), key))
                .or_insert_with(|| SummaryAcc::new(base_labels.to_vec()));
            acc.sample_count = Some(value as u64);
            return true;
        }
    }
    false
}

fn label_pairs(labels: &[(String, String)]) -> Vec<LabelPair> {
    labels
        .iter()
        .map(|(k, v)| {
            let mut lp = LabelPair::default();
            lp.set_name(k.clone());
            lp.set_value(v.clone());
            lp
        })
        .collect()
}

fn labels_key(labels: &[(String, String)]) -> String {
    let mut sorted: Vec<(&String, &String)> = labels.iter().map(|(k, v)| (k, v)).collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    let mut out = String::new();
    for (k, v) in sorted {
        out.push_str(k);
        out.push('=');
        out.push_str(v);
        out.push('\u{1f}'); // unit separator — unlikely to appear in label values
    }
    out
}

fn new_family(name: &str, ty: MetricType) -> MetricFamily {
    let mut f = MetricFamily::default();
    f.set_name(name.to_string());
    f.set_field_type(ty);
    f
}

pub fn push_counter(
    families: &mut BTreeMap<String, MetricFamily>,
    name: &str,
    labels: &[(String, String)],
    value: f64,
) {
    let family = families
        .entry(name.to_string())
        .or_insert_with(|| new_family(name, MetricType::COUNTER));
    let mut counter = Counter::default();
    counter.set_value(value);
    let mut metric = Metric::from_label(label_pairs(labels));
    metric.set_counter(counter);
    family.mut_metric().push(metric);
}

pub fn push_gauge(
    families: &mut BTreeMap<String, MetricFamily>,
    name: &str,
    labels: &[(String, String)],
    value: f64,
) {
    let family = families
        .entry(name.to_string())
        .or_insert_with(|| new_family(name, MetricType::GAUGE));
    let mut gauge = Gauge::default();
    gauge.set_value(value);
    let mut metric = Metric::from_label(label_pairs(labels));
    metric.set_gauge(gauge);
    family.mut_metric().push(metric);
}

pub fn push_untyped(
    families: &mut BTreeMap<String, MetricFamily>,
    name: &str,
    labels: &[(String, String)],
    value: f64,
) {
    let family = families
        .entry(name.to_string())
        .or_insert_with(|| new_family(name, MetricType::UNTYPED));
    let mut untyped = Untyped::default();
    untyped.set_value(value);
    let mut metric = Metric::from_label(label_pairs(labels));
    metric.untyped = MessageField::some(untyped);
    family.mut_metric().push(metric);
}

/// Convenience for callers that pass `&str` slices (synthetic relay metrics).
pub fn push_gauge_strs(
    families: &mut BTreeMap<String, MetricFamily>,
    name: &str,
    labels: &[(&str, &str)],
    value: f64,
) {
    let owned: Vec<(String, String)> = labels
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    push_gauge(families, name, &owned, value);
}

#[derive(Debug)]
struct HistogramAcc {
    labels: Vec<(String, String)>,
    buckets: Vec<(f64, f64)>,
    sample_sum: Option<f64>,
    sample_count: Option<u64>,
}

impl HistogramAcc {
    fn new(labels: Vec<(String, String)>) -> Self {
        Self {
            labels,
            buckets: Vec::new(),
            sample_sum: None,
            sample_count: None,
        }
    }

    fn into_metric(self) -> Metric {
        let mut histogram = Histogram::default();
        let mut buckets = Vec::with_capacity(self.buckets.len());
        // Skip the explicit +Inf bucket if the daemon emitted one — Rust's
        // `f64::to_string()` formats `f64::INFINITY` as `"inf"`, while the
        // prometheus crate's text encoder writes its own canonical
        // `le="+Inf"` line whenever no positive-infinity bucket is present.
        // We use that built-in behaviour, and fall back to the +Inf bucket's
        // cumulative count for `sample_count` if no explicit `_count` line
        // was seen.
        let mut inf_count: Option<u64> = None;
        let mut max_count: u64 = 0;
        for (upper, count) in self.buckets {
            let count_u = count as u64;
            if upper.is_sign_positive() && upper.is_infinite() {
                inf_count = Some(count_u);
                if count_u > max_count {
                    max_count = count_u;
                }
                continue;
            }
            let mut b = Bucket::default();
            b.set_cumulative_count(count_u);
            b.set_upper_bound(upper);
            if count_u > max_count {
                max_count = count_u;
            }
            buckets.push(b);
        }
        histogram.set_bucket(buckets);
        if let Some(sum) = self.sample_sum {
            histogram.set_sample_sum(sum);
        }
        let count = self
            .sample_count
            .or(inf_count)
            .unwrap_or(max_count);
        histogram.set_sample_count(count);

        let mut metric = Metric::from_label(label_pairs(&self.labels));
        metric.set_histogram(histogram);
        metric
    }
}

#[derive(Debug)]
struct SummaryAcc {
    labels: Vec<(String, String)>,
    quantiles: Vec<(f64, f64)>,
    sample_sum: Option<f64>,
    sample_count: Option<u64>,
}

impl SummaryAcc {
    fn new(labels: Vec<(String, String)>) -> Self {
        Self {
            labels,
            quantiles: Vec::new(),
            sample_sum: None,
            sample_count: None,
        }
    }

    fn into_metric(self) -> Metric {
        let mut summary = Summary::default();
        let mut quantiles = Vec::with_capacity(self.quantiles.len());
        for (q, v) in self.quantiles {
            let mut quant = Quantile::default();
            quant.set_quantile(q);
            quant.set_value(v);
            quantiles.push(quant);
        }
        summary.set_quantile(quantiles);
        if let Some(sum) = self.sample_sum {
            summary.set_sample_sum(sum);
        }
        if let Some(count) = self.sample_count {
            summary.set_sample_count(count);
        }
        let mut metric = Metric::from_label(label_pairs(&self.labels));
        metric.set_summary(summary);
        metric
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(body: &str, instance: &str, host: &str, cluster: &str) -> String {
        let families =
            parse_and_relabel(body, instance, host, cluster, "test-cluster").expect("parse ok");
        let bytes = encode_families(&families).expect("encode ok");
        String::from_utf8(bytes).expect("utf8")
    }

    #[test]
    fn counter_keeps_type_and_gets_federation_labels() {
        let body = "# TYPE foo counter\nfoo 7\n";
        let out = render(body, "abc", "host1", "cust1");
        assert!(out.contains("# TYPE foo counter"));
        assert!(out.contains("foo{"));
        assert!(out.contains("instance_id=\"abc\""));
        assert!(out.contains("hostname=\"host1\""));
        assert!(out.contains("cluster_id=\"cust1\""));
        assert!(out.contains("} 7"));
    }

    #[test]
    fn gauge_keeps_type() {
        let body = "# TYPE temp gauge\ntemp{room=\"a\"} 23.5\n";
        let out = render(body, "i", "h", "c");
        assert!(out.contains("# TYPE temp gauge"));
        assert!(out.contains("room=\"a\""));
        assert!(out.contains("instance_id=\"i\""));
        assert!(out.contains("cluster_id=\"c\""));
    }

    #[test]
    fn federation_label_overrides_collision() {
        let body = "# TYPE foo counter\nfoo{instance_id=\"old\"} 1\n";
        let out = render(body, "new", "h", "c");
        assert!(out.contains("instance_id=\"new\""));
        assert!(!out.contains("instance_id=\"old\""));
    }

    #[test]
    fn histogram_round_trips_with_sum_and_count() {
        let body = "\
# TYPE http_request_duration_seconds histogram
http_request_duration_seconds_bucket{le=\"0.1\"} 1
http_request_duration_seconds_bucket{le=\"1\"} 2
http_request_duration_seconds_bucket{le=\"+Inf\"} 3
http_request_duration_seconds_sum 1.5
http_request_duration_seconds_count 3
";
        let out = render(body, "i", "h", "c");
        assert!(out.contains("# TYPE http_request_duration_seconds histogram"));
        assert!(out.contains("http_request_duration_seconds_bucket{"));
        assert!(out.contains("le=\"0.1\""));
        assert!(out.contains("le=\"1\""));
        assert!(out.contains("le=\"+Inf\""));
        assert!(out.contains("http_request_duration_seconds_sum{"));
        assert!(out.contains("http_request_duration_seconds_count{"));
        // Federation labels appear on every emitted line.
        for line in out
            .lines()
            .filter(|l| l.starts_with("http_request_duration_seconds"))
        {
            assert!(line.contains("instance_id=\"i\""), "missing in {line}");
            assert!(line.contains("cluster_id=\"c\""), "missing in {line}");
            assert!(line.contains("hostname=\"h\""), "missing in {line}");
        }
    }

    #[test]
    fn summary_round_trips_with_sum_and_count() {
        let body = "\
# TYPE rpc_duration_seconds summary
rpc_duration_seconds{quantile=\"0.5\"} 4700
rpc_duration_seconds{quantile=\"0.9\"} 9000
rpc_duration_seconds{quantile=\"0.99\"} 76656
rpc_duration_seconds_sum 17560473
rpc_duration_seconds_count 2693
";
        let out = render(body, "i", "h", "c");
        assert!(out.contains("# TYPE rpc_duration_seconds summary"));
        assert!(out.contains("quantile=\"0.5\""));
        assert!(out.contains("quantile=\"0.9\""));
        assert!(out.contains("quantile=\"0.99\""));
        assert!(out.contains("rpc_duration_seconds_sum{"));
        assert!(out.contains("rpc_duration_seconds_count{"));
        for line in out.lines().filter(|l| l.starts_with("rpc_duration_seconds")) {
            assert!(line.contains("instance_id=\"i\""));
            assert!(line.contains("cluster_id=\"c\""));
        }
    }

    #[test]
    fn empty_hostname_emits_empty_string() {
        let body = "# TYPE foo gauge\nfoo 1\n";
        let out = render(body, "i", "", "c");
        assert!(out.contains("hostname=\"\""));
    }

    #[test]
    fn synthetic_helpers_round_trip() {
        let mut families = BTreeMap::new();
        push_gauge_strs(
            &mut families,
            "mac_mgmt_relay_scrape_up",
            &[
                ("instance_id", "abc"),
                ("hostname", "h"),
                ("cluster_id", "c"),
            ],
            1.0,
        );
        push_gauge_strs(
            &mut families,
            "mac_mgmt_relay_scrape_targets",
            &[],
            3.0,
        );
        let out =
            String::from_utf8(encode_families(&families.into_values().collect::<Vec<_>>()).unwrap())
                .unwrap();
        assert!(out.contains("mac_mgmt_relay_scrape_up{"));
        assert!(out.contains("instance_id=\"abc\""));
        assert!(out.contains("mac_mgmt_relay_scrape_targets 3"));
    }
}
