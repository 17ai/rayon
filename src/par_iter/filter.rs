use super::*;
use super::len::*;
use super::state::*;
use super::util::PhantomType;

pub struct Filter<M, FILTER_OP> {
    base: M,
    filter_op: FILTER_OP,
}

impl<M, FILTER_OP> Filter<M, FILTER_OP> {
    pub fn new(base: M, filter_op: FILTER_OP) -> Filter<M, FILTER_OP> {
        Filter { base: base, filter_op: filter_op }
    }
}

impl<M, FILTER_OP> ParallelIterator for Filter<M, FILTER_OP>
    where M: ParallelIterator,
          FILTER_OP: Fn(&M::Item) -> bool + Sync,
{
    type Item = M::Item;
    type Shared = FilterShared<M, FILTER_OP>;
    type State = FilterState<M, FILTER_OP>;

    fn drive<C: Consumer<Item=Self::Item>>(self, consumer: C, shared: C::Shared) -> C::Result {
        let consumer1: FilterConsumer<C, FILTER_OP> = FilterConsumer::new(consumer);
        let shared1 = (shared, self.filter_op);
        self.base.drive(consumer1, shared1)
    }

    fn state(self) -> (Self::Shared, Self::State) {
        let (base_shared, base_state) = self.base.state();
        (FilterShared { base: base_shared, filter_op: self.filter_op },
         FilterState { base: base_state, filter_op: PhantomType::new() })
    }
}

unsafe impl<M, FILTER_OP> BoundedParallelIterator for Filter<M, FILTER_OP>
    where M: BoundedParallelIterator,
          FILTER_OP: Fn(&M::Item) -> bool + Sync
{
    fn upper_bound(&mut self) -> usize {
        self.base.upper_bound()
    }
}

pub struct FilterShared<M, FILTER_OP>
    where M: ParallelIterator,
{
    base: M::Shared,
    filter_op: FILTER_OP,
}

pub struct FilterState<M, FILTER_OP>
    where M: ParallelIterator,
{
    base: M::State,
    filter_op: PhantomType<FILTER_OP>,
}

unsafe impl<M, FILTER_OP> ParallelIteratorState for FilterState<M, FILTER_OP>
    where M: ParallelIterator,
          FILTER_OP: Fn(&M::Item) -> bool + Sync
{
    type Item = M::Item;
    type Shared = FilterShared<M, FILTER_OP>;

    fn len(&mut self, shared: &Self::Shared) -> ParallelLen {
        self.base.len(&shared.base)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (FilterState { base: left, filter_op: PhantomType::new() },
         FilterState { base: right, filter_op: PhantomType::new() })
    }

    fn next(&mut self, shared: &Self::Shared) -> Option<Self::Item> {
        while let Some(base) = self.base.next(&shared.base) {
            if (shared.filter_op)(&base) {
                return Some(base);
            }
        }
        None
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct FilterConsumer<C, FILTER_OP>
    where C: Consumer, FILTER_OP: Fn(&C::Item) -> bool + Sync,
{
    base: C,
    filter_op: PhantomType<FILTER_OP>,
}

impl<C, FILTER_OP> FilterConsumer<C, FILTER_OP>
    where C: Consumer, FILTER_OP: Fn(&C::Item) -> bool + Sync,
{
    fn new(base: C) -> FilterConsumer<C, FILTER_OP> {
        FilterConsumer { base: base, filter_op: PhantomType::new() }
    }
}

impl<C, FILTER_OP> Consumer for FilterConsumer<C, FILTER_OP>
    where C: Consumer, FILTER_OP: Fn(&C::Item) -> bool + Sync,
{
    type Item = C::Item;
    type Shared = (C::Shared, FILTER_OP);
    type SeqState = C::SeqState;
    type Result = C::Result;

    /// Cost to process `items` number of items.
    fn cost(&mut self, shared: &Self::Shared, cost: f64) -> f64 {
        self.base.cost(&shared.0, cost) * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, shared: &Self::Shared, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(&shared.0, index);
        (FilterConsumer::new(left), FilterConsumer::new(right))
    }

    unsafe fn start(&mut self, shared: &Self::Shared) -> C::SeqState {
        self.base.start(&shared.0)
    }

    unsafe fn consume(&mut self,
                      shared: &Self::Shared,
                      state: C::SeqState,
                      item: Self::Item)
                      -> C::SeqState
    {
        if (shared.1)(&item) {
            self.base.consume(&shared.0, state, item)
        } else {
            state
        }
    }

    unsafe fn complete(self, shared: &Self::Shared, state: C::SeqState) -> C::Result {
        self.base.complete(&shared.0, state)
    }

    unsafe fn reduce(shared: &Self::Shared, left: C::Result, right: C::Result) -> C::Result {
        C::reduce(&shared.0, left, right)
    }
}
