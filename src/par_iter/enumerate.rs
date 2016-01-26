use super::*;
use super::state::*;

pub struct Enumerate<M> {
    base: M,
}

impl<M> Enumerate<M> {
    pub fn new(base: M) -> Enumerate<M> {
        Enumerate { base: base }
    }
}

impl<M> ParallelIterator for Enumerate<M>
    where M: PullParallelIterator,
{
    type Item = (usize, M::Item);

    fn drive<'c, C: Consumer<'c, Item=Self::Item>>(self,
                                                   consumer: C,
                                                   shared: &'c C::Shared)
                                                   -> C::Result {
        bridge(self, consumer, &shared)
    }
}

unsafe impl<M> BoundedParallelIterator for Enumerate<M>
    where M: PullParallelIterator,
{
    fn upper_bound(&mut self) -> usize {
        self.len()
    }
}

unsafe impl<M> ExactParallelIterator for Enumerate<M>
    where M: PullParallelIterator,
{
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<M> PullParallelIterator for Enumerate<M>
    where M: PullParallelIterator,
{
    type Producer = EnumerateProducer<M::Producer>;

    fn into_producer(self) -> (Self::Producer, <Self::Producer as Producer>::Shared) {
        let (base, shared) = self.base.into_producer();
        (EnumerateProducer { base: base, offset: 0 }, shared)
    }
}

///////////////////////////////////////////////////////////////////////////
// Producer implementation

pub struct EnumerateProducer<M>
    where M: Producer,
{
    base: M,
    offset: usize,
}

impl<M> Producer for EnumerateProducer<M>
    where M: Producer
{
    type Item = (usize, M::Item);
    type Shared = M::Shared;

    fn cost(&mut self, shared: &Self::Shared, items: usize) -> f64 {
        self.base.cost(shared, items) // enumerating is basically free
    }

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (EnumerateProducer { base: left, offset: self.offset },
         EnumerateProducer { base: right, offset: self.offset + index })
    }

    unsafe fn produce(&mut self, shared: &Self::Shared) -> (usize, M::Item) {
        let item = self.base.produce(shared);
        let index = self.offset;
        self.offset += 1;
        (index, item)
    }
}
