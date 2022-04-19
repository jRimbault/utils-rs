use std::time::Duration;

#[test]
fn basic() {
    let config = include_str!("basic.toml");
    let config: uptime_config::ConfigSerde = toml::from_str(config).unwrap();
    let config = config.usable();
    let servers = [
        ("foo", &[1, 2][..], Some(Duration::from_millis(10)), None),
        ("bar", &[80, 443, 4444], None, Some(Duration::from_secs(10))),
        ("quz", &[80], None, None),
    ];
    for server in servers {
        let (name, ports, interval, period) = server;
        let server = &config.servers[name];
        assert_eq!(server.ports, ports);
        assert_eq!(server.interval, interval);
        assert_eq!(server.period, period);
    }
}
