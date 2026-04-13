use pingwatch::types::Hostname;
use rstest::rstest;

// IP literals bypass DNS entirely and round-trip through to_string().
#[rstest]
#[case("127.0.0.1")]
#[case("::1")]
#[tokio::test]
async fn ip_literal_resolves_to_itself(#[case] input: &str) {
    let h: Hostname = input.parse().unwrap();
    let addr = h.resolve().await.unwrap();
    assert_eq!(addr.to_string(), input);
}

// DNS fallback path: localhost is universally resolvable to a loopback address.
#[tokio::test]
async fn localhost_resolves_to_loopback() {
    let h: Hostname = "localhost".parse().unwrap();
    let addr = h.resolve().await.unwrap();
    assert!(addr.is_loopback(), "expected loopback, got {addr}");
}

// Error path: a syntactically valid but non-existent hostname must fail.
#[tokio::test]
async fn invalid_hostname_returns_error() {
    let h: Hostname = "this.host.definitely.does.not.exist.invalid.xyz"
        .parse()
        .unwrap();
    let result = h.resolve().await;
    assert!(result.is_err(), "expected DNS error, got: {result:?}");
}

// Display and as_str must both round-trip the original input unchanged.
#[rstest]
#[case("example.com")]
#[case("my-host.local")]
#[case("127.0.0.1")]
fn string_roundtrip(#[case] input: &str) {
    let h: Hostname = input.parse().unwrap();
    assert_eq!(h.to_string(), input);
    assert_eq!(h.as_str(), input);
}
