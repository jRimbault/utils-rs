mod bucket;
mod minheap;

use std::collections::HashMap;

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
    K: core::hash::Hash + Eq,
{
    pub fn add(&mut self, value: K) -> Node<K>
    where
        K: Clone,
    {
        let number = self.map.entry(value.clone()).or_default().add_one();
        Node { value, number }
    }

    pub fn remove(&mut self, value: K, number: usize) -> Option<()> {
        self.map.get_mut(&value)?.remove(number)
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
