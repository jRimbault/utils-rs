//! End-to-end test that a sustained ping failure triggers a `notify-send`
//! desktop alert.
//!
//! Instead of asserting against a real notification daemon, this shims
//! `notify-send` with a script that appends its arguments to a log file, then
//! points the spawned binary's `PATH` at the shim. Driving the actual binary
//! (rather than calling `run` in-process) exercises the whole chain: CLI
//! parsing, the worker ping loop, the printer's failure handling, and finally
//! the subprocess spawn.
//!
//! Requires permission to open an ICMP socket (CAP_NET_RAW, or an unprivileged
//! `net.ipv4.ping_group_range`). CI only runs `cargo check`, so this test is
//! exercised locally where that permission is available.

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// TEST-NET-1 (RFC 5737): a resolvable but guaranteed non-routable address, so
/// the worker keeps the ping loop alive while every ping fails.
const UNREACHABLE_HOST: &str = "192.0.2.1";

/// Install a `notify-send` shim into `dir` that records each invocation's
/// arguments (one line per call) into the file named by `$NOTIFY_LOG`.
fn write_notify_shim(dir: &Path) {
    let shim = dir.join("notify-send");
    fs::write(
        &shim,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$NOTIFY_LOG\"\n",
    )
    .expect("write notify-send shim");
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("chmod notify-send shim");
}

#[test]
fn sustained_failure_invokes_notify_send() {
    let workdir = tempfile::tempdir().expect("temp workdir");
    let shim_dir = workdir.path().join("bin");
    let config_home = workdir.path().join("config");
    let log_path = workdir.path().join("notify.log");
    let stderr_path = workdir.path().join("pingwatch.stderr");
    fs::create_dir_all(&shim_dir).expect("create shim dir");
    fs::create_dir_all(&config_home).expect("create config home");
    write_notify_shim(&shim_dir);

    // Prepend the shim directory so the spawned binary finds our fake
    // `notify-send` first. An empty XDG_CONFIG_HOME keeps any real user config
    // from leaking into the run.
    let path = format!(
        "{}:{}",
        shim_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    // Aggressive timing so the second failure -- the one that crosses the
    // notify-after threshold -- happens within a few hundred milliseconds.
    let mut child = Command::new(env!("CARGO_BIN_EXE_pingwatch"))
        .args([
            "--notify-after",
            "50",
            "--interval",
            "200",
            "--timeout",
            "200",
            UNREACHABLE_HOST,
        ])
        .env("PATH", path)
        .env("XDG_CONFIG_HOME", &config_home)
        .env("NOTIFY_LOG", &log_path)
        .stdout(Stdio::null())
        .stderr(Stdio::from(
            fs::File::create(&stderr_path).expect("create stderr capture"),
        ))
        .spawn()
        .expect("spawn pingwatch binary");

    // Poll the log until the down alert lands, the child dies early, or we
    // give up. The loop is the runtime equivalent of "wait for the outage to
    // be declared" without a brittle fixed sleep.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut log = String::new();
    let mut exited_early = false;
    while Instant::now() < deadline {
        log = fs::read_to_string(&log_path).unwrap_or_default();
        if log.contains("is unreachable") {
            break;
        }
        if child.try_wait().expect("poll child").is_some() {
            exited_early = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = child.kill();
    let _ = child.wait();

    let stderr = fs::read_to_string(&stderr_path).unwrap_or_default();
    assert!(
        !exited_early,
        "pingwatch exited before notifying (no ICMP socket permission?). stderr:\n{stderr}"
    );
    assert!(
        log.contains(&format!("{UNREACHABLE_HOST} is unreachable")),
        "expected a down notification for {UNREACHABLE_HOST}, got log:\n{log}\nstderr:\n{stderr}"
    );
    assert!(
        log.contains("--urgency critical"),
        "down notification should use critical urgency, got log:\n{log}"
    );
}
