mod bucket;
mod minheap;

use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct EvictMap {
    map: HashMap<String, bucket::Bucket>,
}

#[derive(Debug, Clone)]
pub struct Node {
    hostname: String,
    number: usize,
}

impl EvictMap {
    pub fn add(&mut self, hostname: &str) -> Node {
        let hostname = hostname.to_owned();
        let number = self.map.entry(hostname.clone()).or_default().add_one();
        Node { hostname, number }
    }

    pub fn remove(&mut self, hostname: &str, number: usize) -> Option<()> {
        self.map.get_mut(hostname)?.remove(number)
    }
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.hostname, self.number)?;
        Ok(())
    }
}
