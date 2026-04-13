use pingwatch::cli::Args;
use rstest::rstest;
use std::path::Path;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write a minimal config file at `<config_home>/pingwatch/config.toml`.
fn write_config(config_home: &Path, content: &str) {
    let dir = config_home.join("pingwatch");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), content).unwrap();
}

/// Call `Args::parse_from("pingwatch", argv)` with `XDG_CONFIG_HOME` pointing
/// at an empty temp directory so config-file values never leak into CLI-only tests.
fn parse_no_config(argv: &[&str]) -> anyhow::Result<Args> {
    let dir = tempfile::tempdir().unwrap();
    // SAFETY: single-threaded test binary; no other thread reads this var concurrently.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.path()) };
    Args::parse_from("pingwatch", argv)
}

// ---------------------------------------------------------------------------
// Host count — enforced after CLI+config merge
// ---------------------------------------------------------------------------

// 0 hosts from CLI with no config → rejected.
// 1–10 from CLI → accepted.
// 11 from CLI → rejected by clap's num_args upper bound.
#[rstest]
#[case(0, false)]
#[case(1, true)]
#[case(10, true)]
#[case(11, false)]
fn host_count_validation(#[case] count: usize, #[case] valid: bool) {
    let hosts: Vec<String> = (1..=count).map(|i| format!("host{i}")).collect();
    let argv: Vec<&str> = std::iter::once("pingwatch")
        .chain(hosts.iter().map(String::as_str))
        .collect();
    assert_eq!(
        parse_no_config(&argv).is_ok(),
        valid,
        "count={count} should be valid={valid}"
    );
}

// ---------------------------------------------------------------------------
// Timing flags — clap-level parsing (no config)
// ---------------------------------------------------------------------------

// Defaults, short forms, and long forms all parse to the correct Duration.
#[rstest]
#[case(&["pingwatch", "host"],                          1000, 2000)]
#[case(&["pingwatch", "--interval", "500", "host"],      500, 2000)]
#[case(&["pingwatch", "-i",         "500", "host"],      500, 2000)]
#[case(&["pingwatch", "--timeout", "3000", "host"],     1000, 3000)]
#[case(&["pingwatch", "-t",        "3000", "host"],     1000, 3000)]
fn timing_params(#[case] argv: &[&str], #[case] interval_ms: u64, #[case] timeout_ms: u64) {
    let args = parse_no_config(argv).unwrap();
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
    assert!(parse_no_config(argv).is_err());
}

// ---------------------------------------------------------------------------
// Config file — hosts
// ---------------------------------------------------------------------------

#[test]
fn config_hosts_used_when_none_on_cli() {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path(), "hosts = [\"example.com\"]\n");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.path()) };

    let args = Args::parse_from("pingwatch", &["pingwatch"]).unwrap();
    assert_eq!(args.hosts.len(), 1);
    assert_eq!(args.hosts[0].as_str(), "example.com");
}

#[test]
fn cli_hosts_override_config_hosts() {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path(), "hosts = [\"config-host.example\"]\n");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.path()) };

    let args = Args::parse_from("pingwatch", &["pingwatch", "cli-host.example"]).unwrap();
    assert_eq!(args.hosts.len(), 1);
    assert_eq!(args.hosts[0].as_str(), "cli-host.example");
}

// ---------------------------------------------------------------------------
// Config file — timing resolution
// ---------------------------------------------------------------------------

// Config values fill in timing when the corresponding flag is absent.
// An empty config string exercises the "no overrides → built-in defaults" path.
#[rstest]
#[case("interval = 500\n",               500, 2000)]
#[case("timeout = 750\n",               1000,  750)]
#[case("interval = 300\ntimeout = 400\n", 300,  400)]
#[case("",                              1000, 2000)]
fn config_timing_used_when_flags_absent(
    #[case] config: &str,
    #[case] expected_interval_ms: u64,
    #[case] expected_timeout_ms: u64,
) {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path(), config);
    unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.path()) };

    let args = Args::parse_from("pingwatch", &["pingwatch", "host"]).unwrap();
    assert_eq!(args.interval.as_millis() as u64, expected_interval_ms);
    assert_eq!(args.timeout.as_millis() as u64, expected_timeout_ms);
}

// CLI timing flags always win over config, even when the CLI value equals the
// built-in default (the key case the old comparison-based merge got wrong).
#[rstest]
#[case("interval = 500\n", &["pingwatch", "--interval", "1000", "host"], 1000, 2000)]
#[case("interval = 500\n", &["pingwatch", "--interval",  "200", "host"],  200, 2000)]
#[case("timeout = 750\n",  &["pingwatch", "--timeout",  "3000", "host"], 1000, 3000)]
fn cli_timing_overrides_config(
    #[case] config: &str,
    #[case] argv: &[&str],
    #[case] expected_interval_ms: u64,
    #[case] expected_timeout_ms: u64,
) {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path(), config);
    unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.path()) };

    let args = Args::parse_from("pingwatch", argv).unwrap();
    assert_eq!(args.interval.as_millis() as u64, expected_interval_ms);
    assert_eq!(args.timeout.as_millis() as u64, expected_timeout_ms);
}

// Zero-value timing in the config file must be rejected.
#[rstest]
#[case("interval = 0\n")]
#[case("timeout = 0\n")]
fn invalid_config_timing_rejected(#[case] config: &str) {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path(), config);
    unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.path()) };

    assert!(Args::parse_from("pingwatch", &["pingwatch", "host"]).is_err());
}
