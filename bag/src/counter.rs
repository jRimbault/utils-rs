use core::hash::Hash;
use core::iter::{Extend, FromIterator};
use std::collections::HashMap;

pub struct Counter<T>(HashMap<T, usize>);

impl<T> Default for Counter<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T> FromIterator<T> for Counter<T>
where
    T: Hash + Eq,
{
    fn from_iter<I>(values: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        let mut counter = Self::default();
        counter.extend(values);
        counter
    }
}

impl<T> Extend<T> for Counter<T>
where
    T: Hash + Eq,
{
    fn extend<I>(&mut self, values: I)
    where
        I: IntoIterator<Item = T>,
    {
        for value in values {
            *self.0.entry(value).or_default() += 1;
        }
    }
}

use std::fmt;
impl<T> fmt::Debug for Counter<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
