
use super::internal::*;
use super::*;
use std::cmp;
use std::iter;

pub struct ZipEq<A: IndexedParallelIterator, B: IndexedParallelIterator> {
    zip: Zip<A, B>
}

/// Create a new `ZipEq` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
#[inline]
pub fn new<A, B>(a: A, b: B) -> ZipEq<A, B>
    where A: IndexedParallelIterator,
          B: IndexedParallelIterator
{
    ZipEq { zip: zip::new(a, b) }
}

impl<A, B> ParallelIterator for ZipEq<A, B>
    where A: IndexedParallelIterator,
          B: IndexedParallelIterator
{
    type Item = (A::Item, B::Item);

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self.zip, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.zip.len())
    }
}

impl<A, B> IndexedParallelIterator for ZipEq<A, B>
    where A: IndexedParallelIterator,
          B: IndexedParallelIterator
{
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self.zip, consumer)
    }

    fn len(&mut self) -> usize {
        self.zip.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        self.zip.with_producer(callback)
    }
}
