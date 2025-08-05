use crate::{
    dust::insert_sort,
    util::{
        cycle_swap, insert_left, insert_right, ptr_sub, rotate, search_left, search_right, Less,
    },
};

/// Holds the state of an internal buffer
pub struct Buffer<T> {
    /// Pointer to the leftmost element in the buffer
    pub start: *mut T,

    /// Number of elements in the buffer
    pub len: usize,

    /// Upper bound on the length of the unsorted prefix
    pub unsorted: usize,
}

impl<T> Buffer<T> {
    // Insert the element pointed to by `item` into this buffer at index `index`, first rotating the
    // buffer to the right of it.
    unsafe fn insert(&mut self, item: *mut T, index: usize) {
        rotate(item.add(1), ptr_sub(self.start, item) - 1, self.len);
        self.start = item;
        self.len += 1;
        insert_right(item, index);
    }

    /// Shift the buffer so that it starts at `dst`.
    pub unsafe fn shift(&mut self, dst: *mut T) {
        if dst < self.start {
            rotate(dst, ptr_sub(self.start, dst), self.len);
        } else {
            rotate(self.start, self.len, ptr_sub(dst, self.start));
        }

        self.start = dst;
    }

    /// Restore the ascending order of the buffer.
    pub fn sort<F: Less<T>>(&mut self, less: &mut F) {
        const MIN_BINARY_INSERT: usize = 128;

        unsafe {
            if self.unsorted > MIN_BINARY_INSERT {
                insert_sort(self.start, 1, MIN_BINARY_INSERT, less);

                for i in MIN_BINARY_INSERT..self.unsorted {
                    let cur = self.start.add(i);
                    insert_left(cur, i - search_right(self.start, i, cur, less));
                }
            } else {
                insert_sort(self.start, 1, self.unsorted, less);
            }
        }
    }

    /// Begin a merge operation by swapping `cnt` buffer elements into position at `dst`.
    pub unsafe fn begin_merge(&mut self, dst: *mut T, cnt: usize) {
        // Detect ord violations by enforcing non-zero merges
        if cnt == 0 {
            panic!("Ord violated");
        }

        self.unsorted = usize::max(self.unsorted, cnt);
        cycle_swap(self.start, dst, cnt);
    }

    /// Search `s..i` from the right to identify unique keys, stopping at `ideal` keys. Use binary
    /// search on this buffer for each element.
    pub unsafe fn binary_find_keys<F: Less<T>>(
        &mut self,
        s: *mut T,
        mut i: *mut T,
        ideal: usize,
        less: &mut F,
    ) {
        while i > s && self.len < ideal {
            i = i.sub(1);
            let pos = search_left(self.start, self.len, i, less);

            if pos == self.len || less(&*i, &*self.start.add(pos)) {
                self.insert(i, pos);
            }
        }
    }

    /// Search `s..i` from the right to identify unique keys, stopping at `ideal` keys. The range is
    /// guaranteed to be sorted, so we compare values efficiently with a virtual merge operation.
    pub unsafe fn block_find_keys<F: Less<T>>(
        &mut self,
        s: *mut T,
        mut i: *mut T,
        ideal: usize,
        less: &mut F,
    ) {
        // The simplest way to adapt to longer runs is to binary search the right bound `i - 1`; the
        // algorithm by design terminates early on the left bound `s`.
        let upper_bound = search_left(self.start, self.len - 1, i.sub(1), less) + 1;

        // Collect keys with a virtual merge
        let mut b = self.start.add(upper_bound);

        while i > s && b > self.start && self.len < ideal {
            if less(&*i.sub(1), &*b.sub(1)) {
                b = b.sub(1);
            } else {
                i = i.sub(1);

                if less(&*b.sub(1), &*i) {
                    b = b.sub(ptr_sub(self.start, i) - 1);
                    self.insert(i, ptr_sub(b, i) - 1);
                }
            }
        }

        // After comparing with the entire buffer, collect new minimums
        while i > s && self.len < ideal {
            i = i.sub(1);

            if less(&*i, &*self.start) {
                self.insert(i, 0);
            }
        }
    }
}
