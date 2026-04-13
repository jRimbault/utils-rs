use clap::Parser;
use pingwatch::cli::Args;
use rstest::rstest;

// Host count: clap enforces 1–10 via num_args.
#[rstest]
#[case(0, false)]
#[case(1, true)]
#[case(10, true)]
#[case(11, false)]
fn host_count_validation(#[case] count: usize, #[case] valid: bool) {
    let hosts: Vec<String> = (1..=count).map(|i| format!("host{i}")).collect();
    let cmd: Vec<&str> = std::iter::once("pingwatch")
        .chain(hosts.iter().map(String::as_str))
        .collect();
    assert_eq!(
        Args::try_parse_from(cmd).is_ok(),
        valid,
        "count={count} should be valid={valid}"
    );
}

// Timing flags: defaults, short forms, and long forms all parse to the correct Duration.
#[rstest]
#[case(&["pingwatch", "host"],                          1000, 2000)]
#[case(&["pingwatch", "--interval", "500", "host"],      500, 2000)]
#[case(&["pingwatch", "-i",         "500", "host"],      500, 2000)]
#[case(&["pingwatch", "--timeout", "3000", "host"],     1000, 3000)]
#[case(&["pingwatch", "-t",        "3000", "host"],     1000, 3000)]
fn timing_params(#[case] argv: &[&str], #[case] interval_ms: u64, #[case] timeout_ms: u64) {
    let args = Args::try_parse_from(argv).unwrap();
    assert_eq!(args.interval.as_millis() as u64, interval_ms);
    assert_eq!(args.timeout.as_millis() as u64, timeout_ms);
}

// Inputs that must be rejected: missing hosts, zero durations, non-numeric values.
#[rstest]
#[case(&["pingwatch"])]
#[case(&["pingwatch", "--interval", "0",   "host"])]
#[case(&["pingwatch", "--timeout",  "0",   "host"])]
#[case(&["pingwatch", "--interval", "abc", "host"])]
#[case(&["pingwatch", "--timeout",  "1s",  "host"])]
fn invalid_args_rejected(#[case] argv: &[&str]) {
    assert!(Args::try_parse_from(argv).is_err());
}
