use std::time::Duration;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
pub struct Ticker {
    limit: Option<Duration>,
    interval: Duration,
}

impl Ticker {
    pub fn builder() -> TickerBuilder {
        TickerBuilder::new()
    }
}

#[derive(Debug)]
pub struct TickerBuilder {
    limit: Option<Duration>,
    interval: Option<Duration>,
}

impl TickerBuilder {
    fn new() -> TickerBuilder {
        TickerBuilder {
            limit: None,
            interval: None,
        }
    }

    pub fn interval(mut self, interval: Duration) -> Self {
        self.interval = Some(interval);
        self
    }

    pub fn limit(mut self, limit: Duration) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn build(self) -> Result<Ticker> {
        let TickerBuilder { limit, interval } = self;
        let interval = interval.ok_or_else(|| Error::MissingInterval)?;
        if let Some(limit) = limit {
            if limit < interval {
                return Err(Error::IntervalLargerThanLimit { limit, interval });
            }
        }
        Ok(Ticker { limit, interval })
    }
}

#[derive(Debug)]
pub enum Error {
    MissingInterval,
    IntervalLargerThanLimit { limit: Duration, interval: Duration },
}

pub mod iter {
    use std::time::{Duration, Instant};

    use crate::Ticker;

    #[derive(Debug)]
    pub struct IntoIter {
        start: Instant,
        limit: Option<Duration>,
        ticker: crossbeam_channel::IntoIter<Instant>,
    }

    impl IntoIterator for Ticker {
        type Item = Instant;
        type IntoIter = IntoIter;

        fn into_iter(self) -> Self::IntoIter {
            let Ticker { limit, interval } = self;
            IntoIter {
                limit,
                start: Instant::now(),
                ticker: crossbeam_channel::tick(interval).into_iter(),
            }
        }
    }

    impl Iterator for IntoIter {
        type Item = Instant;

        fn next(&mut self) -> Option<Self::Item> {
            match self.limit {
                Some(limit) if self.start.elapsed() > limit => None,
                _ => self.ticker.next(),
            }
        }
    }
}
