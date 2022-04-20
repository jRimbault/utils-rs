use bitvec::vec::BitVec;
use conv::ValueFrom;

/// Keep the original floating point value, display it as percentage.
#[derive(Clone, Copy)]
pub struct Percent(f64);

#[derive(Debug)]
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
    pub fn success_rate(&self) -> Result<Percent, conv::GeneralErrorKind> {
        success_rate(&self.0)
    }
    pub fn successes(&self) -> usize {
        self.0.count_ones()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[derive(Debug)]
pub struct RollingStats(BitVec);

impl RollingStats {
    pub fn with_capacity(capacity: usize) -> RollingStats {
        let mut list = BitVec::with_capacity(capacity);
        for _ in 0..capacity {
            list.push(true);
        }
        RollingStats(list)
    }
    pub fn add(&mut self, value: bool) {
        let len = self.0.len();
        self.0.shift_left(1);
        self.0.resize(len - 1, true);
        self.0.push(value)
    }
    pub fn success_rate(&self) -> Result<Percent, conv::GeneralErrorKind> {
        success_rate(&self.0)
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

fn success_rate(list: &BitVec) -> Result<Percent, conv::GeneralErrorKind> {
    let successes = f64::value_from(list.count_ones())?;
    let total = f64::value_from(list.len())?;
    Ok(Percent(successes / total))
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
