use std::{cmp::Reverse, collections::BinaryHeap};

pub struct MinHeap<T>(BinaryHeap<Reverse<T>>);

impl<T> MinHeap<T>
where
    T: Ord,
{
    pub fn push(&mut self, value: T) {
        self.0.push(Reverse(value));
    }

    pub fn pop(&mut self) -> Option<T> {
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

impl<T> Default for MinHeap<T>
where
    T: Ord,
{
    fn default() -> Self {
        MinHeap(Default::default())
    }
}
