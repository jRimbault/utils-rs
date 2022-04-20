use std::time::Duration;

use ticker::{Error, Ticker};

#[test]
fn missing_interval() {
    let ticker = Ticker::builder().build();
    assert!(ticker.is_err());
    let error = ticker.unwrap_err();
    assert!(matches!(error, ticker::Error::MissingInterval));
    let ticker = Ticker::builder().limit(Default::default()).build();
    assert!(ticker.is_err());
    let error = ticker.unwrap_err();
    assert!(matches!(error, ticker::Error::MissingInterval));
}

#[test]
fn interval_bigger_than_limit() {
    let ticker = Ticker::builder()
        .limit(Duration::from_micros(1))
        .interval(Duration::from_secs(1))
        .build();
    assert!(ticker.is_err());
    let error = ticker.unwrap_err();
    assert!(matches!(error, Error::IntervalLargerThanLimit));
}

#[test]
fn tick_around_10_times() -> ticker::Result<()> {
    let ticker = Ticker::builder()
        .limit(Duration::from_millis(200))
        .interval(Duration::from_millis(20))
        .build()?;
    let ticks = ticker.into_iter().count();
    assert!(ticks <= 11);
    assert!(ticks >= 9);
    Ok(())
}
