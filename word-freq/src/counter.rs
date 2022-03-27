use priority_queue::PriorityQueue;

#[derive(Debug)]
pub struct SortedCounter<T>(pub priority_queue::PriorityQueue<T, usize>)
where
    T: core::hash::Hash + Eq;

impl<T> FromIterator<T> for SortedCounter<T>
where
    T: core::hash::Hash + Eq,
{
    fn from_iter<I>(values: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        let mut counter = PriorityQueue::new();
        for item in values {
            let mut found = false;
            counter.change_priority_by(&item, |n| {
                found = true;
                *n += 1;
            });
            if !found {
                counter.push(item, 1);
            }
        }
        Self(counter)
    }
}
