use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap};

#[derive(Debug, Default)]
pub struct EvictMap {
    map: HashMap<String, Bucket>,
}

#[derive(Debug, Default)]
struct Bucket {
    allocated: BTreeSet<usize>,
    deallocated: MinHeap<usize>,
}

#[derive(Debug, Clone)]
pub struct Node {
    hostname: String,
    number: usize,
}

impl EvictMap {
    pub fn add(&mut self, hostname: &str) -> Node {
        let hostname = hostname.to_owned();
        let bucket = self.map.entry(hostname.clone()).or_default();
        let number = bucket.allocated.len();
        let number = bucket.deallocated.pop().unwrap_or(number);
        bucket.allocated.insert(number);
        Node { hostname, number }
    }

    pub fn remove(&mut self, hostname: &str, number: usize) -> Option<()> {
        let bucket = self.map.get_mut(hostname)?;
        if !bucket.allocated.contains(&number) {
            return None;
        }
        bucket.allocated.remove(&number);
        bucket.deallocated.push(number);
        Some(())
    }
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.hostname, self.number)?;
        Ok(())
    }
}

#[derive(Default)]
struct MinHeap<T: Ord>(BinaryHeap<Reverse<T>>);

impl<T> MinHeap<T>
where
    T: Ord,
{
    fn push(&mut self, value: T) {
        self.0.push(Reverse(value));
    }

    fn pop(&mut self) -> Option<T> {
        self.0.pop().map(|Reverse(value)| value)
    }
}

impl<T> std::fmt::Debug for MinHeap<T>
where
    T: std::fmt::Debug,
    T: Ord,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.0.iter().map(|Reverse(value)| value))
            .finish()
    }
}
