use join;
use super::len::*;

/// The trait for types representing the internal *state* during
/// parallelization. This basically represents a group of tasks
/// to be done.
///
/// Note that this trait is declared as an **unsafe trait**. That
/// means that the trait is unsafe to implement. The reason is that
/// other bits of code, such as the `collect` routine on
/// `ParallelIterator`, rely on the `len` and `for_each` functions
/// being accurate and correct. For example, if the `len` function
/// reports that it will produce N items, then `for_each` *must*
/// produce `N` items or else the resulting vector will contain
/// uninitialized memory.
///
/// This trait is not really intended to be implemented outside of the
/// Rayon crate at this time. The precise safety requirements are kind
/// of ill-documented for this reason (i.e., they are ill-understood).
pub unsafe trait ParallelIteratorState: Sized {
    type Item;
    type Shared: Sync;

    /// Returns an estimate of how much work is to be done.
    ///
    /// # Safety note
    ///
    /// If sparse is false, then `maximal_len` must be precisely
    /// correct.
    fn len(&mut self, shared: &Self::Shared) -> ParallelLen;

    /// Split this state into two other states, ideally of roughly
    /// equal size.
    fn split_at(self, index: usize) -> (Self, Self);

    /// Extract the next item from this iterator state. Once this
    /// method is called, sequential iteration has begun, and the
    /// other methods will no longer be called.
    fn next(&mut self, shared: &Self::Shared) -> Option<Self::Item>;
}

/// A producer which will produce a fixed number of items N. This is
/// not queryable through the API; the consumer is expected to track
/// it.
pub trait Producer: Send {
    type Item;
    type Shared: Sync;
    type SeqState;

    /// Split into two producers; one produces items `0..index`, the
    /// other `index..N`. Index must be less than `N`.
    unsafe fn split_at(self, index: u64) -> (Self, Self);

    /// Start producing items. Returns some sequential state that must
    /// be threaded.
    unsafe fn start(&mut self, shared: &Self::Shared) -> Self::SeqState;

    /// Unless a panic occurs, expects to be called *exactly N times*.
    unsafe fn produce(&mut self, shared: &Self::Shared, state: &mut Self::SeqState) -> Self::Item;

    /// Finish producing items.
    unsafe fn complete(self, shared: &Self::Shared, state: Self::SeqState);
}

pub trait Consumer: Send {
    type Item;
    type Shared: Sync;
    type SeqState;
    type Result: Send;

    /// When splitting, this is the type of the left consumer. It is
    /// almost always `Self`, but separating out the type helps ensure
    /// that generic code that drives consumers is more correct, since
    /// they cannot mix up the results from the left and right side.
    type Left: Consumer<Item=Self::Item, Shared=Self::Shared>;

    /// See `Left`.
    type Right: Consumer<Item=Self::Item, Shared=Self::Shared>;

    unsafe fn split_at(self, index: u64) -> (Self::Left, Self::Right);
    unsafe fn start(&mut self, shared: &Self::Shared) -> Self::SeqState;
    unsafe fn consume(&mut self,
                      shared: &Self::Shared,
                      state: Self::SeqState,
                      item: Self::Item)
                      -> Self::SeqState;
    unsafe fn complete(self, shared: &Self::Shared, state: Self::SeqState) -> Self::Result;
    unsafe fn reduce(shared: &Self::Shared,
                     left: <Self::Left as Consumer>::Result,
                     right: <Self::Right as Consumer>::Result)
                     -> Self::Result;
}

pub fn bridge<P,C>(len: u64,
                   cost: f64,
                   mut producer: P,
                   producer_shared: &P::Shared,
                   mut consumer: C,
                   consumer_shared: &C::Shared)
                   -> C::Result
    where P: Producer, C: Consumer<Item=P::Item>
{
    unsafe { // asserting that we call the `op` methods in correct pattern
        if len > 1 && cost > THRESHOLD {
            let mid = len / 2;
            let (left_producer, right_producer) = producer.split_at(mid);
            let (left_consumer, right_consumer) = consumer.split_at(mid);
            let (left_result, right_result) =
                join(|| bridge(mid, cost / 2.0,
                               left_producer, producer_shared,
                               left_consumer, consumer_shared),
                     || bridge(len - mid, cost / 2.0,
                               right_producer, producer_shared,
                               right_consumer, consumer_shared));
            C::reduce(consumer_shared, left_result, right_result)
        } else {
            let mut producer_state = producer.start(producer_shared);
            let mut consumer_state = consumer.start(consumer_shared);
            for _ in 0..len {
                let item = producer.produce(producer_shared, &mut producer_state);
                consumer_state = consumer.consume(consumer_shared, consumer_state, item);
            }
            producer.complete(producer_shared, producer_state);
            consumer.complete(consumer_shared, consumer_state)
        }
    }
}
