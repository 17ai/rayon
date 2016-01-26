use super::*;
use super::len::*;
use super::state::*;
use super::util::PhantomType;

pub struct FilterMap<M, FILTER_OP> {
    base: M,
    filter_op: FILTER_OP,
}

impl<M, FILTER_OP> FilterMap<M, FILTER_OP> {
    pub fn new(base: M, filter_op: FILTER_OP) -> FilterMap<M, FILTER_OP> {
        FilterMap { base: base, filter_op: filter_op }
    }
}

impl<M, FILTER_OP, R> ParallelIterator for FilterMap<M, FILTER_OP>
    where M: ParallelIterator,
          FILTER_OP: Fn(M::Item) -> Option<R> + Sync,
          R: Send,
{
    type Item = R;

    fn drive_stateless<'c, C: StatelessConsumer<'c, Item=Self::Item>>(self,
                                                                      consumer: C,
                                                                      shared: &'c C::Shared)
                                                                      -> C::Result {
        let consumer: FilterMapConsumer<M::Item, C, FILTER_OP> = FilterMapConsumer::new(consumer);
        let shared = (shared, &self.filter_op);
        self.base.drive_stateless(consumer, &shared)
    }
}

unsafe impl<M, FILTER_OP, R> BoundedParallelIterator for FilterMap<M, FILTER_OP>
    where M: BoundedParallelIterator,
          FILTER_OP: Fn(M::Item) -> Option<R> + Sync,
          R: Send,
{
    fn upper_bound(&mut self) -> usize {
        self.base.upper_bound()
    }

    fn drive<'c, C: Consumer<'c, Item=Self::Item>>(self,
                                                   consumer: C,
                                                   shared: &'c C::Shared)
                                                   -> C::Result {
        let consumer: FilterMapConsumer<M::Item, C, FILTER_OP> = FilterMapConsumer::new(consumer);
        let shared = (shared, &self.filter_op);
        self.base.drive(consumer, &shared)
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct FilterMapConsumer<'f, 'c, ITEM, C, FILTER_OP>
    where C: Consumer<'c>, FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync, 'c: 'f
{
    base: C,
    phantoms: PhantomType<(&'f &'c (), ITEM, FILTER_OP)>,
}

impl<'f, 'c, ITEM, C, FILTER_OP> FilterMapConsumer<'f, 'c, ITEM, C, FILTER_OP>
    where C: Consumer<'c>, FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync, 'c: 'f
{
    fn new(base: C) -> FilterMapConsumer<'f, 'c, ITEM, C, FILTER_OP> {
        FilterMapConsumer { base: base, phantoms: PhantomType::new() }
    }
}

impl<'f, 'c, ITEM, C, FILTER_OP> Consumer<'f> for FilterMapConsumer<'f, 'c, ITEM, C, FILTER_OP>
    where C: Consumer<'c>, FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync, ITEM: 'f, FILTER_OP: 'f,
{
    type Item = ITEM;
    type Shared = (&'c C::Shared, &'f FILTER_OP);
    type SeqState = C::SeqState;
    type Result = C::Result;

    /// Cost to process `items` number of items.
    fn cost(&mut self, shared: &Self::Shared, cost: f64) -> f64 {
        self.base.cost(&shared.0, cost) * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, shared: &Self::Shared, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(&shared.0, index);
        (FilterMapConsumer::new(left), FilterMapConsumer::new(right))
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
        if let Some(mapped_item) = (shared.1)(item) {
            self.base.consume(&shared.0, state, mapped_item)
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

impl<'f, 'c, ITEM, C, FILTER_OP> StatelessConsumer<'f> for FilterMapConsumer<'f, 'c, ITEM, C, FILTER_OP>
    where C: StatelessConsumer<'c>, FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync, ITEM: 'f, FILTER_OP: 'f,
{
    fn split(&self) -> Self {
        FilterMapConsumer::new(self.base.split())
    }
}

