//! Sampling-based failure discovery with automatic minimization.
//!
//! This test runs many short chaos rounds searching for invariant
//! violations. When a failure is found, it automatically attempts to
//! minimize the schedule to the smallest reproducing case.
//!
//! Control via env vars:
//!   SAMPLE_ROUNDS=100     Number of rounds to run (default: 10)
//!   SAMPLE_EVENTS=10      Fault events per round (default: 8)
//!   SAMPLE_DAEMONS=2      Number of daemons (default: 2)
//!   SAMPLE_SEED=12345     Base seed for deterministic runs

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string()),
        )
        .try_init();
}

fn env_or<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

#[tokio::test]
async fn failure_sampling() {
    init_tracing();

    let rounds: usize = env_or("SAMPLE_ROUNDS", 10);
    let events: usize = env_or("SAMPLE_EVENTS", 8);
    let daemons: usize = env_or("SAMPLE_DAEMONS", 2);
    let base_seed: u64 = env_or("SAMPLE_SEED", sim_tests::scenarios::random_seed());

    eprintln!(
        "sampling: {rounds} rounds, {events} events/round, {daemons} daemon(s), base_seed={base_seed}"
    );

    let report = sim_tests::sampler::sample(base_seed, rounds, events, daemons).await;

    eprintln!("{report}");

    assert!(
        report.failures.is_empty(),
        "{} failure(s) found in {rounds} rounds. See report above for reproduction seeds.\n\
         Re-run specific failures with CHAOS_SEED=<seed>",
        report.failures.len()
    );
}
