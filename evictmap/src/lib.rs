mod bucket;

use core::hash::Hash;
use std::{borrow::Borrow, collections::HashMap};

#[derive(Debug, Default)]
pub struct EvictMap<K> {
    map: HashMap<K, bucket::Bucket>,
}

#[derive(Debug, Clone)]
pub struct Node<T> {
    value: T,
    number: usize,
}

impl<K> EvictMap<K>
where
    K: Hash + Eq,
{
    pub fn add(&mut self, value: K) -> Node<K>
    where
        K: Clone,
    {
        let number = self.map.entry(value.clone()).or_default().add_one();
        Node { value, number }
    }

    pub fn remove<Q>(&mut self, value: &Q, number: usize) -> Option<()>
    where
        Q: ?Sized,
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.map.get_mut(value)?.remove(number)
    }
}

impl<T> std::fmt::Display for Node<T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.value, self.number)?;
        Ok(())
    }
}
