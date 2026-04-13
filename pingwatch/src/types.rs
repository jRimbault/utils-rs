//! Domain-level newtypes shared across the crate.

/// A hostname or IP-address string, validated at the CLI boundary.
#[derive(Clone, Debug)]
pub struct Hostname(String);

impl Hostname {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Hostname {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::str::FromStr for Hostname {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Hostname(s.to_string()))
    }
}

/// Index of a host's slot in the current run's host list.
///
/// Constructed once in `lib::run` from the enumeration position; all
/// subsequent indexing into `bars` and `hosts` slices goes through this type
/// to prevent confusing it with an unrelated `usize`.
#[derive(Clone, Copy, Debug)]
pub struct HostIdx(usize);

impl HostIdx {
    pub fn new(i: usize) -> Self {
        Self(i)
    }

    pub fn as_usize(self) -> usize {
        self.0
    }
}
