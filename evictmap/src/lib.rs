mod bucket;

use core::hash::Hash;
use std::{borrow::Borrow, collections::HashMap};

#[derive(Debug, Default)]
pub struct EvictMap<K> {
    map: HashMap<K, bucket::Bucket>,
}

impl<K> EvictMap<K>
where
    K: Hash + Eq,
{
    pub fn add(&mut self, value: K) -> usize
    where
        K: Clone,
    {
        self.map.entry(value.clone()).or_default().add_one()
    }

    pub fn remove<Q>(&mut self, value: &Q, number: usize) -> bool
    where
        Q: ?Sized,
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.map
            .get_mut(value)
            .and_then(|bucket| bucket.remove(number))
            .is_some()
    }
}

#[cfg(test)]
mod test {
    use super::EvictMap;

    #[test]
    fn scenario_1() {
        let mut map = EvictMap::default();
        assert_eq!(map.add("apibox"), 0);
        assert_eq!(map.add("apibox"), 1);
        assert_eq!(map.add("apibox"), 2);
        assert_eq!(map.add("sitebox"), 0);
        assert!(!map.remove("apibox", 3));
        assert!(map.remove("apibox", 1)); // remove 1 first
        assert!(map.remove("apibox", 0)); // then 0
        assert_eq!(map.add("apibox"), 0); // get back 0 first
        assert_eq!(map.add("apibox"), 1); // then 1
        println!("{map:#?}");
    }
}
