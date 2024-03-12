pub trait IteratorUtils: Iterator + Sized {
    /// Promote an iterator into an exact size iterator, when you know it's exact len
    fn into_exact_size_iterator(self, len: usize) -> ExactSizeIteratorWrapper<Self> {
        ExactSizeIteratorWrapper { it: self, len }
    }
}

impl<I: Iterator> IteratorUtils for I {}

/// An ExactSizeIterator that wrap an Iterator with a known size.
pub struct ExactSizeIteratorWrapper<It: Iterator> {
    it: It,
    len: usize,
}

impl<It: Iterator> Iterator for ExactSizeIteratorWrapper<It> {
    type Item = It::Item;
    
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.it.next();
        // if next.is_none() {
        //     assert!(self.len == 0, "internal error: the number of elements should be set exactly when creating the iterator (some elements were not consumed)");
        // } else {
        //     assert!(self.len > 0, "internal error: the number of elements should be set exactly when creating the iterator ({} extra elements were expected to be consumed)", self.len);
        // }
        self.len -= 1;
        next
    }
    
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<It: Iterator> ExactSizeIterator for ExactSizeIteratorWrapper<It> {}
