use crate::{
    spinner_style::{DEFAULT_SPINNER_STYLE_NAME, SpinnerStyle},
    types::Hostname,
};
use anyhow::Context as _;
use std::time::Duration;

/// Config file schema — all fields optional; CLI takes precedence.
///
/// Located at `$XDG_CONFIG_HOME/<bin>/config.toml`,
/// falling back to `~/.config/<bin>/config.toml`.
/// Timing fields are plain millisecond integers; `Duration` has no native
/// TOML representation.
#[derive(serde::Deserialize, Default)]
struct Config {
    hosts: Option<Vec<Hostname>>,
    /// Interval between pings in milliseconds.
    interval: Option<u64>,
    /// Per-ping timeout in milliseconds.
    timeout: Option<u64>,
    /// Silence threshold in milliseconds: a host that fails to respond for at
    /// least this long triggers a desktop notification.
    notify_after: Option<u64>,
    /// Spinner style preset name. Animated builds expose the full catalog;
    /// static builds accept only `staticDot`.
    spinner_style: Option<SpinnerStyle>,
}

/// Ping one or more hosts simultaneously, showing live status in a TUI.
///
/// Hosts are read exclusively from the TOML config file (there is no limit on
/// how many may be listed). All other arguments can also be persisted there
/// so they do not have to be repeated on every invocation; CLI arguments take
/// precedence over config file values for those.
///
/// Config file location (in order of precedence):
///
///   $XDG_CONFIG_HOME/pingwatch/config.toml
///   ~/.config/pingwatch/config.toml
///
/// Supported keys:
///
///   hosts        = ["example.com", "8.8.8.8"]   # list of hostnames or IPs
///   interval     = 1000                         # milliseconds between pings
///   timeout      = 2000                         # per-ping timeout in milliseconds
///   notify_after = 30000                        # silence before a desktop notification (ms)
///   spinner_style = "staticDot"                 # or any animated preset when the feature is enabled
// The derive keeps the clap API intact (including `try_parse_from` used in
// tests); the inherent `parse(bin_name)` method shadows it for production
// use and adds config-file resolution.
#[derive(clap::Parser)]
#[command(version, verbatim_doc_comment)]
pub struct Args {
    /// Hosts to ping, supplied only through the config file.
    #[arg(skip)]
    pub hosts: Vec<Hostname>,
    /// Interval between pings in milliseconds
    #[arg(short, long, default_value = "1000", value_parser = parse_millis)]
    pub interval: Duration,
    /// Per-ping timeout in milliseconds
    #[arg(short, long, default_value = "2000", value_parser = parse_millis)]
    pub timeout: Duration,
    /// Notify via `notify-send` once a host has been silent this many milliseconds
    #[arg(short = 'n', long, default_value = "30000", value_parser = parse_millis)]
    pub notify_after: Duration,
    /// Spinner style preset from cli-spinners
    #[arg(long, value_enum, default_value = DEFAULT_SPINNER_STYLE_NAME)]
    pub spinner_style: SpinnerStyle,
}

impl Args {
    /// Parse and resolve configuration from `std::env::args_os()` and the XDG
    /// config file. Delegates to [`Args::parse_from`].
    pub fn parse(bin_name: &str) -> anyhow::Result<Self> {
        Self::parse_from(bin_name, std::env::args_os())
    }

    /// Parse and resolve configuration from the given argv and the XDG config file.
    ///
    /// Resolution order: CLI > config file > built-in defaults.
    /// Accepts any iterator of `OsString`-convertible values so tests can supply
    /// a controlled argv without touching `std::env::args`.
    pub fn parse_from<I, T>(bin_name: &str, argv: I) -> anyhow::Result<Self>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let config = load_config(bin_name)?;

        let matches = <Self as clap::CommandFactory>::command()
            .try_get_matches_from(argv)
            .map_err(|e| {
                // Help and version requests are informational, not errors — print
                // them and exit cleanly rather than surfacing as anyhow errors.
                if !e.use_stderr() {
                    e.exit();
                }
                anyhow::Error::from(e)
            })?;

        let hosts = config.hosts.unwrap_or_default();
        anyhow::ensure!(
            !hosts.is_empty(),
            "at least one host is required (add a `hosts` entry to the config file)"
        );

        let interval = resolve_duration(&matches, "interval", config.interval, 1000)?;
        let timeout = resolve_duration(&matches, "timeout", config.timeout, 2000)?;
        let notify_after = resolve_duration(&matches, "notify_after", config.notify_after, 30000)?;
        let spinner_style = resolve_spinner_style(
            &matches,
            "spinner_style",
            config.spinner_style,
            SpinnerStyle::default(),
        );

        Ok(Args {
            hosts,
            interval,
            timeout,
            notify_after,
            spinner_style,
        })
    }
}

/// Resolve a timing argument: CLI (explicit) > config file value > built-in default.
///
/// Checks `ValueSource` to distinguish an explicit `--flag VALUE` from a value
/// that clap filled in from `default_value`. This avoids the broken pattern of
/// comparing the parsed value to the known default, which would misfire when the
/// user explicitly passes the default value.
fn resolve_duration(
    matches: &clap::ArgMatches,
    name: &str,
    config_ms: Option<u64>,
    default_ms: u64,
) -> anyhow::Result<Duration> {
    use clap::parser::ValueSource;

    if matches.value_source(name) == Some(ValueSource::CommandLine) {
        // The user explicitly passed the flag — trust clap's parsed value.
        return Ok(*matches
            .get_one::<Duration>(name)
            .expect("CommandLine source guarantees a value"));
    }

    if let Some(ms) = config_ms {
        anyhow::ensure!(ms > 0, "config: `{name}` must be at least 1 ms, got {ms}");
        return Ok(Duration::from_millis(ms));
    }

    Ok(Duration::from_millis(default_ms))
}

/// Resolve an enum argument: CLI (explicit) > config file value > built-in default.
fn resolve_spinner_style(
    matches: &clap::ArgMatches,
    name: &str,
    config_style: Option<SpinnerStyle>,
    default_style: SpinnerStyle,
) -> SpinnerStyle {
    use clap::parser::ValueSource;

    if matches.value_source(name) == Some(ValueSource::CommandLine) {
        return *matches
            .get_one::<SpinnerStyle>(name)
            .expect("CommandLine source guarantees a value");
    }

    config_style.unwrap_or(default_style)
}

/// Load and deserialize the TOML config file.
///
/// Returns `Ok(Config::default())` when the file does not exist so callers
/// can treat absence as "no overrides" without special-casing.
fn load_config(bin_name: &str) -> anyhow::Result<Config> {
    let path = xdg_config_dir().join(bin_name).join("config.toml");
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("reading config file {}", path.display()))?;
    toml_edit::de::from_str(&content)
        .with_context(|| format!("parsing config file {}", path.display()))
}

/// Return the XDG config home directory.
///
/// Respects `$XDG_CONFIG_HOME` when it is set to an absolute path;
/// falls back to `$HOME/.config` per the XDG Base Directory Specification.
fn xdg_config_dir() -> std::path::PathBuf {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from)
        && dir.is_absolute()
    {
        return dir;
    }
    let home = std::env::var_os("HOME").unwrap_or_default();
    std::path::PathBuf::from(home).join(".config")
}

fn parse_millis(s: &str) -> Result<Duration, String> {
    let ms: u64 = s
        .parse()
        .map_err(|e: std::num::ParseIntError| e.to_string())?;
    if ms == 0 {
        return Err("value must be at least 1ms".to_string());
    }
    Ok(Duration::from_millis(ms))
}
