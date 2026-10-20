// Each test binary compiles its own copy of `support`; this one never runs
// the full ping loop, so `IntegrationFixture::run` is unused here.
#[expect(
    dead_code,
    reason = "IntegrationFixture::run is only used by integration tests"
)]
mod support;

use pingwatch::cli::Args;
use pingwatch::spinner_style::SpinnerStyle;
use rstest::rstest;
use support::IntegrationFixture;

/// Call `Args::parse_from("pingwatch", argv)` with `XDG_CONFIG_HOME` pointing
/// at an empty temp directory so config-file values never leak into CLI-only tests.
fn parse_no_config(argv: &[&str]) -> anyhow::Result<Args> {
    let fixture = IntegrationFixture::new();
    fixture.parse(argv)
}

// ---------------------------------------------------------------------------
// Host count — hosts come only from the config file, no CLI positional
// ---------------------------------------------------------------------------

// No hosts and no config → rejected.
#[test]
fn no_hosts_anywhere_rejected() {
    assert!(parse_no_config(&["pingwatch"]).is_err());
}

// Hosts passed on the CLI are rejected as unexpected arguments.
#[test]
fn hosts_on_cli_rejected() {
    assert!(parse_no_config(&["pingwatch", "host"]).is_err());
}

// There is no upper bound on the number of hosts a config file can list.
#[rstest]
#[case(1)]
#[case(10)]
#[case(11)]
#[case(250)]
fn any_number_of_config_hosts_accepted(#[case] count: usize) {
    let hosts: Vec<String> = (1..=count).map(|i| format!("\"host{i}\"")).collect();
    let config = format!("hosts = [{}]\n", hosts.join(", "));
    let fixture = IntegrationFixture::with_config(&config);
    let args = fixture.parse(["pingwatch"]).unwrap();
    assert_eq!(args.hosts.len(), count);
}

// ---------------------------------------------------------------------------
// Timing flags — clap-level parsing (no config, hosts supplied by config)
// ---------------------------------------------------------------------------

fn with_one_host() -> IntegrationFixture {
    IntegrationFixture::with_config("hosts = [\"host\"]\n")
}

// Defaults, short forms, and long forms all parse to the correct Duration.
#[rstest]
#[case(&["pingwatch"],                          1000, 2000)]
#[case(&["pingwatch", "--interval", "500"],      500, 2000)]
#[case(&["pingwatch", "-i",         "500"],      500, 2000)]
#[case(&["pingwatch", "--timeout", "3000"],     1000, 3000)]
#[case(&["pingwatch", "-t",        "3000"],     1000, 3000)]
fn timing_params(#[case] argv: &[&str], #[case] interval_ms: u64, #[case] timeout_ms: u64) {
    let fixture = with_one_host();
    let args = fixture.parse(argv).unwrap();
    assert_eq!(args.interval.as_millis() as u64, interval_ms);
    assert_eq!(args.timeout.as_millis() as u64, timeout_ms);
}

// Inputs that must be rejected: zero durations, non-numeric values.
#[rstest]
#[case(&["pingwatch", "--interval", "0"])]
#[case(&["pingwatch", "--timeout",  "0"])]
#[case(&["pingwatch", "--notify-after", "0"])]
#[case(&["pingwatch", "--interval", "abc"])]
#[case(&["pingwatch", "--timeout",  "1s"])]
fn invalid_args_rejected(#[case] argv: &[&str]) {
    let fixture = with_one_host();
    assert!(fixture.parse(argv).is_err());
}

// notify-after: default, short, and long forms all parse to the right Duration.
#[rstest]
#[case(&["pingwatch"],                            30000)]
#[case(&["pingwatch", "--notify-after", "5000"],   5000)]
#[case(&["pingwatch", "-n",             "5000"],   5000)]
fn notify_after_param(#[case] argv: &[&str], #[case] expected_ms: u64) {
    let fixture = with_one_host();
    let args = fixture.parse(argv).unwrap();
    assert_eq!(args.notify_after.as_millis() as u64, expected_ms);
}

// ---------------------------------------------------------------------------
// Spinner style — clap-level parsing and config precedence
// ---------------------------------------------------------------------------

#[cfg(feature = "animated-spinners")]
#[rstest]
#[case(&["pingwatch"], SpinnerStyle::Dots14)]
#[case(&["pingwatch", "--spinner-style", "star"], SpinnerStyle::Star)]
#[case(&["pingwatch", "--spinner-style", "dots8Bit"], SpinnerStyle::Dots8Bit)]
fn spinner_style_params(#[case] argv: &[&str], #[case] expected: SpinnerStyle) {
    let fixture = with_one_host();
    let args = fixture.parse(argv).unwrap();
    assert_eq!(args.spinner_style, expected);
}

#[cfg(not(feature = "animated-spinners"))]
#[test]
fn static_dot_is_the_default_spinner_style() {
    let fixture = with_one_host();
    let args = fixture.parse(["pingwatch"]).unwrap();
    assert_eq!(args.spinner_style, SpinnerStyle::StaticDot);
}

#[rstest]
#[case(&["pingwatch", "--spinner-style", "not-a-style"])]
fn invalid_spinner_style_rejected(#[case] argv: &[&str]) {
    let fixture = with_one_host();
    assert!(fixture.parse(argv).is_err());
}

// ---------------------------------------------------------------------------
// Config file — hosts
// ---------------------------------------------------------------------------

#[test]
fn config_hosts_used() {
    let fixture = IntegrationFixture::with_config("hosts = [\"example.com\"]\n");
    let args = fixture.parse(["pingwatch"]).unwrap();
    assert_eq!(args.hosts.len(), 1);
    assert_eq!(args.hosts[0].as_str(), "example.com");
}

#[test]
fn config_hosts_accept_heterogeneous_entries() {
    let fixture = IntegrationFixture::with_config(
        "hosts = [\"example.com\", \"8.8.8.8\", { name = \"router\", ip = \"192.168.1.1\" }]\n",
    );
    let args = fixture.parse(["pingwatch"]).unwrap();
    let labels: Vec<&str> = args.hosts.iter().map(|h| h.as_str()).collect();
    assert_eq!(labels, ["example.com", "8.8.8.8", "router"]);
}

#[test]
fn config_named_host_with_invalid_ip_rejected() {
    let fixture =
        IntegrationFixture::with_config("hosts = [{ name = \"router\", ip = \"not-an-ip\" }]\n");
    assert!(fixture.parse(["pingwatch"]).is_err());
}

// ---------------------------------------------------------------------------
// Config file — timing resolution
// ---------------------------------------------------------------------------

// Config values fill in timing when the corresponding flag is absent.
// An empty config string exercises the "no overrides → built-in defaults" path.
#[rstest]
#[case("hosts = [\"host\"]\ninterval = 500\n", 500, 2000)]
#[case("hosts = [\"host\"]\ntimeout = 750\n", 1000, 750)]
#[case("hosts = [\"host\"]\ninterval = 300\ntimeout = 400\n", 300, 400)]
#[case("hosts = [\"host\"]\n", 1000, 2000)]
fn config_timing_used_when_flags_absent(
    #[case] config: &str,
    #[case] expected_interval_ms: u64,
    #[case] expected_timeout_ms: u64,
) {
    let fixture = IntegrationFixture::with_config(config);
    let args = fixture.parse(["pingwatch"]).unwrap();
    assert_eq!(args.interval.as_millis() as u64, expected_interval_ms);
    assert_eq!(args.timeout.as_millis() as u64, expected_timeout_ms);
}

// CLI timing flags always win over config, even when the CLI value equals the
// built-in default (the key case the old comparison-based merge got wrong).
#[rstest]
#[case("hosts = [\"host\"]\ninterval = 500\n", &["pingwatch", "--interval", "1000"], 1000, 2000)]
#[case("hosts = [\"host\"]\ninterval = 500\n", &["pingwatch", "--interval",  "200"],  200, 2000)]
#[case("hosts = [\"host\"]\ntimeout = 750\n",  &["pingwatch", "--timeout",  "3000"], 1000, 3000)]
fn cli_timing_overrides_config(
    #[case] config: &str,
    #[case] argv: &[&str],
    #[case] expected_interval_ms: u64,
    #[case] expected_timeout_ms: u64,
) {
    let fixture = IntegrationFixture::with_config(config);
    let args = fixture.parse(argv).unwrap();
    assert_eq!(args.interval.as_millis() as u64, expected_interval_ms);
    assert_eq!(args.timeout.as_millis() as u64, expected_timeout_ms);
}

// notify_after resolves from config when the flag is absent, and the CLI flag
// wins when both are present.
#[test]
fn config_notify_after_used_when_flag_absent() {
    let fixture = IntegrationFixture::with_config("hosts = [\"host\"]\nnotify_after = 5000\n");
    let args = fixture.parse(["pingwatch"]).unwrap();
    assert_eq!(args.notify_after.as_millis() as u64, 5000);
}

#[test]
fn cli_notify_after_overrides_config() {
    let fixture = IntegrationFixture::with_config("hosts = [\"host\"]\nnotify_after = 5000\n");
    let args = fixture
        .parse(["pingwatch", "--notify-after", "10000"])
        .unwrap();
    assert_eq!(args.notify_after.as_millis() as u64, 10000);
}

// Zero-value timing in the config file must be rejected.
#[rstest]
#[case("hosts = [\"host\"]\ninterval = 0\n")]
#[case("hosts = [\"host\"]\ntimeout = 0\n")]
#[case("hosts = [\"host\"]\nnotify_after = 0\n")]
fn invalid_config_timing_rejected(#[case] config: &str) {
    let fixture = IntegrationFixture::with_config(config);
    assert!(fixture.parse(["pingwatch"]).is_err());
}

#[cfg(feature = "animated-spinners")]
#[test]
fn config_spinner_style_used_when_flag_absent() {
    let fixture = IntegrationFixture::with_config("hosts = [\"host\"]\nspinner_style = \"arc\"\n");
    let args = fixture.parse(["pingwatch"]).unwrap();
    assert_eq!(args.spinner_style, SpinnerStyle::Arc);
}

#[cfg(feature = "animated-spinners")]
#[test]
fn cli_spinner_style_overrides_config_even_when_it_matches_the_default() {
    let fixture = IntegrationFixture::with_config("hosts = [\"host\"]\nspinner_style = \"arc\"\n");
    let args = fixture
        .parse(["pingwatch", "--spinner-style", "dots14"])
        .unwrap();
    assert_eq!(args.spinner_style, SpinnerStyle::Dots14);
}

#[cfg(not(feature = "animated-spinners"))]
#[test]
fn config_static_dot_spinner_style_used_when_flag_absent() {
    let fixture =
        IntegrationFixture::with_config("hosts = [\"host\"]\nspinner_style = \"staticDot\"\n");
    let args = fixture.parse(["pingwatch"]).unwrap();
    assert_eq!(args.spinner_style, SpinnerStyle::StaticDot);
}

#[test]
fn invalid_config_spinner_style_rejected() {
    let fixture =
        IntegrationFixture::with_config("hosts = [\"host\"]\nspinner_style = \"not-a-style\"\n");
    assert!(fixture.parse(["pingwatch"]).is_err());
}

// ---------------------------------------------------------------------------
// Config file — explicit `--config` path
// ---------------------------------------------------------------------------

/// Write `content` to a config file outside the fixture's XDG config home, so
/// a test can tell which of the two files was read.
fn custom_config(content: &str) -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().expect("temp config file");
    std::fs::write(file.path(), content).expect("write custom config");
    file
}

// The explicit file replaces the XDG one entirely, for both long and short forms.
#[rstest]
#[case("--config")]
#[case("-c")]
fn explicit_config_replaces_xdg_config(#[case] flag: &str) {
    let fixture = IntegrationFixture::with_config("hosts = [\"xdg-host\"]\ninterval = 500\n");
    let custom = custom_config("hosts = [\"custom-host\"]\n");
    let args = fixture
        .parse([
            "pingwatch",
            flag,
            custom.path().to_str().expect("utf-8 temp path"),
        ])
        .unwrap();
    let labels: Vec<&str> = args.hosts.iter().map(|h| h.as_str()).collect();
    assert_eq!(labels, ["custom-host"]);
    // interval came only from the XDG file, so it must not leak through.
    assert_eq!(args.interval.as_millis() as u64, 1000);
    assert_eq!(args.config.as_deref(), Some(custom.path()));
}

// Works without any XDG config present.
#[test]
fn explicit_config_used_without_xdg_config() {
    let fixture = IntegrationFixture::new();
    let custom = custom_config("hosts = [\"custom-host\"]\ntimeout = 750\n");
    let args = fixture
        .parse(["pingwatch", "--config", custom.path().to_str().unwrap()])
        .unwrap();
    assert_eq!(args.hosts[0].as_str(), "custom-host");
    assert_eq!(args.timeout.as_millis() as u64, 750);
}

// CLI flags keep precedence over values from an explicit config file.
#[test]
fn cli_flags_override_explicit_config() {
    let fixture = IntegrationFixture::new();
    let custom = custom_config("hosts = [\"host\"]\ninterval = 500\n");
    let args = fixture
        .parse([
            "pingwatch",
            "--interval",
            "200",
            "--config",
            custom.path().to_str().unwrap(),
        ])
        .unwrap();
    assert_eq!(args.interval.as_millis() as u64, 200);
}

// A missing explicit file is an error naming the path, even when a valid XDG
// config exists to fall back on.
#[test]
fn missing_explicit_config_rejected() {
    let fixture = IntegrationFixture::with_config("hosts = [\"xdg-host\"]\n");
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nope.toml");
    let err = fixture
        .parse(["pingwatch", "--config", missing.to_str().unwrap()])
        .err()
        .expect("missing --config file should be rejected");
    assert!(
        format!("{err:#}").contains("nope.toml"),
        "error should name the file: {err:#}"
    );
}

// Without the flag, the resolved record carries no explicit path.
#[test]
fn config_path_absent_by_default() {
    let args = with_one_host().parse(["pingwatch"]).unwrap();
    assert!(args.config.is_none());
}
