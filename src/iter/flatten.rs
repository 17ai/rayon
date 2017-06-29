use super::internal::*;
use super::*;

/// `Flatten` turns each element to an iterator, then flattens these iterators
/// together. This struct is created by the [`flatten()`] method on
/// [`ParallelIterator`].
///
/// [`flatten()`]: trait.ParallelIterator.html#method.flatten
/// [`ParallelIterator`]: trait.ParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Flatten<I: ParallelIterator> {
    base: I,
}

/// Create a new `Flatten` iterator.
pub fn new<I>(base: I) -> Flatten<I>
    where I: ParallelIterator
{
    Flatten {
        base: base,
    }
}

impl<I, PI> ParallelIterator for Flatten<I>
    where I: ParallelIterator<Item = PI>,
          PI: IntoParallelIterator + Send + Sync
{
    type Item = PI::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        self.base.flat_map(|x| x).drive_unindexed(consumer)
    }
}
