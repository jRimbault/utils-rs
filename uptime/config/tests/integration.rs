#[test]
fn basic() {
    let config = include_str!("basic.toml");
    let config: uptime_config::Config = toml::from_str(config).unwrap();
    println!("{config:#?}");
}
