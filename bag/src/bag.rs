//! Bag is an ordered map of collections.
//!
//! ```
//! # use bag::Bag;
//! let bag: Bag<i32, &str> = vec![
//!     (3, "hello world"),
//!     (3, "foobar"),
//!     (7, "fizz"),
//!     (7, "buzz"),
//!     (6, "rust"),
//! ].into_iter().collect();
//!
//! assert_eq!(bag[&3], ["hello world", "foobar"]);
//! assert_eq!(bag[&7], ["fizz", "buzz"]);
//! assert_eq!(bag[&6], ["rust"]);
//! ```
#![deny(
    bad_style,
    dead_code,
    improper_ctypes,
    missing_debug_implementations,
    non_shorthand_field_patterns,
    no_mangle_generic_items,
    overflowing_literals,
    path_statements,
    patterns_in_fns_without_body,
    unconditional_recursion,
    unused,
    unused_allocation,
    unused_comparisons,
    unused_parens,
    while_true
)]
#![cfg_attr(
    feature = "more-warnings",
    warn(
        trivial_casts,
        trivial_numeric_casts,
        unused_extern_crates,
        unused_import_braces,
        unused_qualifications,
        unused_results
    )
)]
#![cfg_attr(
    feature = "even-more-warnings",
    warn(missing_copy_implementations, missing_docs)
)]

#[cfg(feature = "serde")]
mod serde;

use std::{
    fmt,
    hash::Hash,
    iter::{Extend, FromIterator},
    ops::Index,
};

use indexmap::Equivalent;

/// Ordered map of collections.
///
/// Similar to python 3.6+ `defaultdict(list)`
pub struct Bag<K, V>(indexmap::IndexMap<K, Vec<V>>);

impl<K, V> Bag<K, V> {
    /// Push `item` at the end of the bucket `key`.
    /// If the bucket doesn't exists, it is created.
    pub fn insert(&mut self, key: K, item: V)
    where
        K: Hash + Eq,
    {
        self.0.entry(key).or_default().push(item);
    }

    /// Borrows the backing [`IndexMap`](indexmap::IndexMap) of the bag.
    pub const fn as_inner(&self) -> &indexmap::IndexMap<K, Vec<V>> {
        &self.0
    }

    /// Mutably borrows the backing [`IndexMap`](indexmap::IndexMap) of the bag.
    pub fn as_inner_mut(&mut self) -> &mut indexmap::IndexMap<K, Vec<V>> {
        &mut self.0
    }

    /// Consumes the wrapper [`Bag`] and returns the inner [`IndexMap`](indexmap::IndexMap).
    pub fn into_inner(self) -> indexmap::IndexMap<K, Vec<V>> {
        self.0
    }

    /// Returns the number of buckets in the bag.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the bag contains no elements.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns a reference to the bucket corresponding to the key.
    pub fn get<Q>(&self, key: &Q) -> Option<&Vec<V>>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.get(key)
    }

    /// Returns a mutable reference to the bucket corresponding to the key.
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut Vec<V>>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.get_mut(key)
    }

    /// Gets the given key’s corresponding entry in the bag for in-place manipulation.
    pub fn entry(&mut self, key: K) -> indexmap::map::Entry<'_, K, Vec<V>>
    where
        K: Hash + Eq,
    {
        self.0.entry(key)
    }
}

impl<K, V> FromIterator<(K, V)> for Bag<K, V>
where
    K: Hash + Eq,
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
    K: Hash + Eq,
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

impl<K, Q, V> Index<&Q> for Bag<K, V>
where
    Q: ?Sized + Hash + Equivalent<K>,
{
    type Output = Vec<V>;

    /// Returns a reference to the value corresponding to the supplied key.
    ///
    /// # Panics
    ///
    /// Panics if the key is not present in the [`Bag`].
    fn index(&self, key: &Q) -> &Self::Output {
        self.get(key).expect("no entry found for key")
    }
}

impl<K, V> Default for Bag<K, V> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<K, V> fmt::Debug for Bag<K, V>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<K, V> From<Bag<K, V>> for indexmap::IndexMap<K, Vec<V>> {
    fn from(value: Bag<K, V>) -> indexmap::IndexMap<K, Vec<V>> {
        value.into_inner()
    }
}

impl<K, V> From<indexmap::IndexMap<K, Vec<V>>> for Bag<K, V> {
    fn from(value: indexmap::IndexMap<K, Vec<V>>) -> Self {
        Self(value)
    }
}

#[cfg(test)]
mod test {
    use indexmap::IndexMap;

    use super::Bag;

    #[test]
    fn test_insert_and_get() {
        let mut bag = Bag::default();
        bag.insert(1, "a");
        bag.insert(1, "b");
        bag.insert(2, "c");

        assert_eq!(bag.get(&1), Some(&vec!["a", "b"]));
        assert_eq!(bag.get(&2), Some(&vec!["c"]));
        assert_eq!(bag.get(&3), None);
    }

    #[test]
    fn test_get_mut() {
        let mut bag = Bag::default();
        bag.insert(1, "a");
        bag.insert(1, "b");

        if let Some(values) = bag.get_mut(&1) {
            values.push("c");
        }

        assert_eq!(bag.get(&1), Some(&vec!["a", "b", "c"]));
    }

    #[test]
    fn test_len_and_is_empty() {
        let mut bag = Bag::default();
        assert_eq!(bag.len(), 0);
        assert!(bag.is_empty());

        bag.insert(1, "a");
        assert_eq!(bag.len(), 1);
        assert!(!bag.is_empty());

        bag.insert(2, "b");
        assert_eq!(bag.len(), 2);
    }

    #[test]
    fn test_as_inner() {
        let mut bag = Bag::default();
        bag.insert(1, "a");
        bag.insert(2, "b");

        let inner: &IndexMap<_, _> = bag.as_inner();
        assert_eq!(inner.len(), 2);
        assert_eq!(inner[&1], vec!["a"]);
        assert_eq!(inner[&2], vec!["b"]);
    }

    #[test]
    fn test_as_inner_mut() {
        let mut bag = Bag::default();
        bag.insert(1, "a");

        let inner_mut: &mut IndexMap<_, _> = bag.as_inner_mut();
        inner_mut.get_mut(&1).unwrap().push("b");

        assert_eq!(bag.get(&1), Some(&vec!["a", "b"]));
    }

    #[test]
    fn test_into_inner() {
        let mut bag = Bag::default();
        bag.insert(1, "a");
        bag.insert(2, "b");

        let inner: IndexMap<_, _> = bag.into_inner();
        assert_eq!(inner.len(), 2);
        assert_eq!(inner[&1], vec!["a"]);
        assert_eq!(inner[&2], vec!["b"]);
    }

    #[test]
    fn test_index() {
        let mut bag = Bag::default();
        bag.insert(1, "a");
        bag.insert(1, "b");

        assert_eq!(bag[&1], vec!["a", "b"]);
    }

    #[test]
    #[should_panic(expected = "no entry found for key")]
    fn test_index_nonexistent() {
        let bag = Bag::<i32, &str>::default();
        let _ = &bag[&1];
    }

    #[test]
    fn test_from_iter() {
        let bag: Bag<_, _> = vec![(1, "a"), (1, "b"), (2, "c")].into_iter().collect();

        assert_eq!(bag.get(&1), Some(&vec!["a", "b"]));
        assert_eq!(bag.get(&2), Some(&vec!["c"]));
    }

    #[test]
    fn test_extend() {
        let mut bag = Bag::default();
        bag.extend(vec![(1, "a"), (1, "b"), (2, "c")]);

        assert_eq!(bag.get(&1), Some(&vec!["a", "b"]));
        assert_eq!(bag.get(&2), Some(&vec!["c"]));
    }

    #[test]
    fn test_from_and_into_indexmap() {
        let mut index_map = IndexMap::new();
        let _ = index_map.insert(1, vec!["a"]);
        let _ = index_map.insert(2, vec!["b"]);

        let bag: Bag<_, _> = index_map.clone().into();
        let new_index_map: IndexMap<_, _> = bag.into();

        assert_eq!(index_map, new_index_map);
    }
}
