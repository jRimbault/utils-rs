use core::iter::{Extend, FromIterator};

/// Ordered map of collections
/// think python 3.6 `defaultdict(list)`
pub struct Bag<K, V>(std::collections::BTreeMap<K, Vec<V>>);

impl<K, V> Bag<K, V> {
    pub fn into_inner(self) -> std::collections::BTreeMap<K, Vec<V>> {
        self.0
    }

    pub fn insert(&mut self, key: K, value: V)
    where
        K: Ord,
    {
        self.0.entry(key).or_default().push(value);
    }
}

impl<K, V> FromIterator<(K, V)> for Bag<K, V>
where
    K: Ord,
{
    fn from_iter<I>(key_value_pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
    {
        let mut bag = Self::default();
        bag.extend(key_value_pairs);
        bag
    }
}

impl<K, V> Extend<(K, V)> for Bag<K, V>
where
    K: Ord,
{
    fn extend<I>(&mut self, key_value_pairs: I)
    where
        I: IntoIterator<Item = (K, V)>,
    {
        for (key, value) in key_value_pairs {
            self.0.entry(key).or_default().push(value);
        }
    }
}

impl<K, V> Default for Bag<K, V> {
    fn default() -> Self {
        Self(Default::default())
    }
}

use std::fmt;
impl<K, V> fmt::Debug for Bag<K, V>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<K, V> From<Bag<K, V>> for std::collections::BTreeMap<K, Vec<V>> {
    fn from(value: Bag<K, V>) -> std::collections::BTreeMap<K, Vec<V>> {
        value.into_inner()
    }
}
