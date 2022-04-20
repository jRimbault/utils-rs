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
//! Building [`Ticker`] without a time limit:
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
    Ticker::builder()
        .interval(interval)
        .build()
        .unwrap()
        .into_iter()
}

#[derive(Debug)]
pub struct Ticker {
    limit: Option<Duration>,
    interval: Duration,
}

impl Ticker {
    /// Helper to safely build a [`Ticker`].
    ///
    /// See [`TickerBuilder`]'s documentation.
    pub fn builder() -> TickerBuilder {
        TickerBuilder::new()
    }
}

/// Helper to safely build a [`Ticker`].
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

    /// *Mandatory*: sets the interval of the ticker.
    pub fn interval(mut self, interval: Duration) -> TickerBuilder {
        self.interval = Some(interval);
        self
    }

    /// *Optional*: sets a time limit on the ticker.
    pub fn limit(mut self, limit: Duration) -> TickerBuilder {
        self.limit = Some(limit);
        self
    }

    /// Check the configuration and builds the [`Ticker`] or an [`Error`].
    pub fn build(self) -> core::result::Result<Ticker, Error> {
        let TickerBuilder { limit, interval } = self;
        let interval = interval.ok_or(Error::MissingInterval)?;
        if interval.is_zero() {
            return Err(Error::IntervalIsZero);
        }
        if let Some(limit) = limit {
            if limit < interval {
                return Err(Error::IntervalLargerThanLimit { limit, interval });
            }
        }
        Ok(Ticker { limit, interval })
    }
}

/// Invalid [`Ticker`] configuration.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("missing interval")]
    MissingInterval,
    #[error("interval can't be zero")]
    IntervalIsZero,
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
