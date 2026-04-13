mod support;

use std::time::Duration;

use support::{FixtureConfig, IntegrationFixture};

const NXDOMAIN_HOST: &str = "this.host.does.not.exist.invalid";
const NXDOMAIN_HOST_2: &str = "this.host.also.does.not.exist.invalid";

#[tokio::test(flavor = "current_thread")]
async fn run_exits_when_cli_host_fails_resolution() {
    let fixture = IntegrationFixture::new();
    fixture.run(["pingwatch", NXDOMAIN_HOST]).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn run_exits_when_config_supplies_host() {
    let fixture = IntegrationFixture::with_config(&format!("hosts = [\"{NXDOMAIN_HOST}\"]\n"));
    fixture.run(["pingwatch"]).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn run_exits_when_multiple_hosts_fail_resolution() {
    let fixture = IntegrationFixture::new();
    fixture
        .run(["pingwatch", NXDOMAIN_HOST, NXDOMAIN_HOST_2])
        .await
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn fixture_configures_timing_and_config_home_for_runtime() {
    let fixture = IntegrationFixture::from_config(FixtureConfig {
        config_toml: Some(&format!(
            "hosts = [\"{NXDOMAIN_HOST}\"]\ninterval = 25\ntimeout = 50\n"
        )),
    });

    let args = fixture.parse(["pingwatch"]).unwrap();
    assert_eq!(args.interval, Duration::from_millis(25));
    assert_eq!(args.timeout, Duration::from_millis(50));
    assert_eq!(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        Some(fixture.config_home().as_os_str())
    );

    pingwatch::run(args).await.unwrap();
}
