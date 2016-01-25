use super::*;
use super::len::ParallelLen;
use super::state::ParallelIteratorState;

pub struct SliceIter<'data, T: 'data + Sync> {
    slice: &'data [T]
}

impl<'data, T: Sync> IntoParallelIterator for &'data [T] {
    type Item = &'data T;
    type Iter = SliceIter<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        SliceIter { slice: self }
    }
}

impl<'data, T: Sync + 'data> IntoParallelRefIterator<'data> for [T] {
    type Item = T;
    type Iter = SliceIter<'data, T>;

    fn par_iter(&'data self) -> Self::Iter {
        self.into_par_iter()
    }
}

impl<'data, T: Sync> ParallelIterator for SliceIter<'data, T> {
    type Item = &'data T;
    type Shared = ();
    type State = Self;

    fn state(self) -> (Self::Shared, Self::State) {
        ((), self)
    }
}

unsafe impl<'data, T: Sync> BoundedParallelIterator for SliceIter<'data, T> { }

unsafe impl<'data, T: Sync> ExactParallelIterator for SliceIter<'data, T> { }

unsafe impl<'data, T: Sync> ParallelIteratorState for SliceIter<'data, T> {
    type Item = &'data T;
    type Shared = ();

    fn len(&mut self, _shared: &Self::Shared) -> ParallelLen {
        ParallelLen {
            maximal_len: self.slice.len(),
            cost: self.slice.len() as f64,
            sparse: false,
        }
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.slice.split_at(index);
        (left.into_par_iter(), right.into_par_iter())
    }

    fn next(&mut self, _shared: &Self::Shared) -> Option<&'data T> {
        self.slice.split_first()
                  .map(|(head, tail)| {
                      self.slice = tail;
                      head
                  })
    }
}
