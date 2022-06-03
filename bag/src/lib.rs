mod bag;
pub mod counter;
mod hashbag;
mod treebag;

pub use bag::Bag as IndexBag;
pub use hashbag::Bag as HashBag;
pub use treebag::Bag as TreeBag;
