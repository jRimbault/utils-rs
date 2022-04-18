use bitvec::vec::BitVec;
use conv::ValueFrom;

/// Keep the original floating point value, display it as percentage.
#[derive(Clone, Copy)]
pub struct Percent(f64);

pub struct Stats(BitVec);

impl Stats {
    pub fn new() -> Stats {
        Stats(BitVec::new())
    }
    pub fn add_success(&mut self) {
        self.0.push(true)
    }
    pub fn add_failure(&mut self) {
        self.0.push(false)
    }
    pub fn uptime_rate(&self) -> Result<Percent, conv::GeneralErrorKind> {
        let successes = f64::value_from(self.0.count_ones())?;
        let total = f64::value_from(self.0.len())?;
        Ok(Percent(successes / total))
    }
    pub fn successes(&self) -> usize {
        self.0.count_ones()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl std::fmt::Display for Percent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (self.0 * 100.).fmt(f)
    }
}
impl std::fmt::Debug for Percent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
