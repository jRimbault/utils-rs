mod minheap;

use std::collections::BTreeSet;

#[derive(Debug, Default)]
pub struct Bucket {
    allocated: BTreeSet<usize>,
    deallocated: minheap::MinHeap<usize>,
}

impl Bucket {
    pub fn add_one(&mut self) -> usize {
        let number = self.next_number();
        self.allocated.insert(number);
        number
    }

    fn next_number(&mut self) -> usize {
        let n = self.allocated.len();
        self.deallocated.pop().unwrap_or(n)
    }

    pub fn remove(&mut self, number: usize) -> Option<()> {
        self.allocated
            .remove(&number)
            .then(|| self.deallocated.push(number))
    }
}
