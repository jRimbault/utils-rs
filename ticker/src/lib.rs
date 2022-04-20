//! Opinionated wrapper around [`crossbeam_channel::tick`].
//!
//! [`Ticker`] is an iterator yielding [`Instant`](`std::time::Instant`)s at regular intervals.
//!
//! # Examples
//!
//! Using a [`Ticker`] to make an iterator yielding every 100 milliseconds for 1 seconds:
//!
//! ```
//! # use std::time::Duration;
//! # use ticker::Ticker;
//! # fn foo() -> ticker::Result<()> {
//! let ticker = Ticker::builder()
//!     .interval(Duration::from_millis(100))
//!     .limit(Duration::from_secs(1))
//!     .build()?;
//! for tick in ticker {
//!     println!("{tick:?}");
//! }
//! # Ok(())
//! # }
//! ```
//!
//! Build [`Ticker`] without a time limit:
//!
//! ```
//! # use std::time::Duration;
//! # use ticker::Ticker;
//! # fn foo() -> ticker::Result<()> {
//! let ticker = Ticker::builder()
//!     .interval(Duration::from_millis(100))
//!     .build()?;
//! for tick in ticker.into_iter().take(5) {
//!     println!("{tick:?}");
//! }
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

pub type Result<T> = core::result::Result<T, Error>;

/// Shortcut for a [`Ticker`] without a time limit.
///
/// # Example
///
/// ```
/// # use std::time::Duration;
/// # use ticker::ticker;
/// # fn foo() -> ticker::Result<()> {
/// for tick in ticker(Duration::from_millis(100)).take(5) {
///     println!("{tick:?}");
/// }
/// # Ok(())
/// # }
/// ```
pub fn ticker(interval: Duration) -> iter::IntoIter {
    iter::IntoIter::new(interval)
}

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
        let interval = interval.ok_or(Error::MissingInterval)?;
        if let Some(limit) = limit {
            if limit < interval {
                return Err(Error::IntervalLargerThanLimit { limit, interval });
            }
        }
        Ok(Ticker { limit, interval })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("missing interval")]
    MissingInterval,
    #[error("interval ({interval:?}) should be smaller than the limit ({limit:?})")]
    IntervalLargerThanLimit { interval: Duration, limit: Duration },
}

#[doc(hidden)]
pub mod iter {
    use std::time::{Duration, Instant};

    use crate::Ticker;

    #[derive(Debug)]
    pub struct IntoIter {
        start: Instant,
        limit: Option<Duration>,
        ticker: crossbeam_channel::IntoIter<Instant>,
    }

    impl IntoIter {
        pub(crate) fn new(interval: Duration) -> IntoIter {
            IntoIter {
                start: Instant::now(),
                limit: None,
                ticker: crossbeam_channel::tick(interval).into_iter(),
            }
        }
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
