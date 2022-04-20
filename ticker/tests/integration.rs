use std::time::{Duration, Instant};

use ticker::{Error, Ticker};

fn ms(milliseconds: u64) -> Duration {
    Duration::from_millis(milliseconds)
}

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
    assert!(matches!(error, Error::IntervalLargerThanLimit { .. }));
}

#[test]
fn tick_around_10_times() -> ticker::Result<()> {
    let ticker = Ticker::builder().limit(ms(200)).interval(ms(20)).build()?;
    let ticks = ticker.into_iter().count();
    assert!(ticks <= 11);
    assert!(ticks >= 10);
    Ok(())
}

#[test]
fn tick_exactly_10_times() -> ticker::Result<()> {
    let ticker = Ticker::builder().interval(ms(20)).build()?;
    let ticks = ticker.into_iter().take(10).count();
    assert_eq!(ticks, 10);
    Ok(())
}

#[test]
fn interval_between_ticks_should_be_consistent() {
    let eq = |a, b| (a + ms(30) > b && b + ms(30) > a);
    let ticker = Ticker::builder().interval(ms(100)).build().unwrap();
    let start = Instant::now();
    // first tick should be immediate
    for (i, tick) in ticker.into_iter().enumerate().take(5) {
        assert!(eq(tick, start + ms(100 * i as u64)));
    }
}
