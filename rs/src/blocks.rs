use core::ptr;

use crate::{
    buffer::Buffer,
    dust::RATIO_BIN_MERGE,
    merge::{binary_merge_left, merge_lazy, merge_left, merge_right},
    util::{block_swap_length, conditional, insert_left, search_left, search_right, Less},
};

// Holds the state of a contiguous sequence of blocks
struct Blocks<T> {
    // Pointer to the start of the buffer that stores block keys
    keys: *mut T,

    // Index of the first key of a B block
    mid: usize,

    // Total number of blocks
    count: usize,
}

impl<T> Blocks<T> {
    // Rearrange runs `s..s + n1` and `s + n1..s + n1 + n2` into a sorted sequence of blocks, saving
    // the state in `keys`. Blocks are of length `block_len`.
    //
    // Return the state of the rearranged blocks.
    unsafe fn sorted_from_runs<F: Less<T>>(
        s: *mut T,
        keys: *mut T,
        n1: usize,
        n2: usize,
        block_len: usize,
        less: &mut F,
    ) -> Self {
        // Number of blocks in the left run
        let c1 = n1 / block_len;

        let mut blocks = Self {
            keys,
            mid: c1,
            count: c1 + n2 / block_len,
        };

        let block_less = {
            let val = s.add(block_len - 1);
            move |i, j, less: &mut F| less(&*val.add(i * block_len), &*val.add(j * block_len))
        };

        let block_swap = |i, j| {
            ptr::swap(keys.add(i), keys.add(j));
            ptr::swap_nonoverlapping(s.add(i * block_len), s.add(j * block_len), block_len);
        };

        let Some(mid) = (0..c1).find(|&i| block_less(c1, i, less)) else {
            return blocks;
        };

        block_swap(mid, c1);

        // Track indices of minimum A and B blocks
        let mut ma = c1;
        let mut mb = c1 + 1;
        let mut i = mid + 1;

        // Optimized selection sort by @aphitorite
        while i < mb {
            if mb == blocks.count || !block_less(mb, ma, less) {
                if ma != i {
                    block_swap(i, ma);
                }

                ma = usize::max(c1, i + 1);

                for j in ma + 1..mb {
                    ma = conditional(ma, j, less(&*keys.add(j), &*keys.add(ma)));
                }
            } else {
                block_swap(i, mb);
                ma = conditional(ma, mb, ma == i);
                mb += 1;
            }

            i += 1;
        }

        blocks.mid = mid;
        blocks
    }

    // Pop the block at index `i`.
    //
    // Return whether or not it came from the left run.
    unsafe fn pop<F: Less<T>>(&mut self, i: usize, less: &mut F) -> bool {
        let block_is_a = i != self.mid && less(&*self.keys.add(i), &*self.keys.add(self.mid));

        if block_is_a {
            insert_left(self.keys.add(i), i - self.mid);
            self.mid += 1;
        }

        block_is_a
    }
}

// Merge runs `s..s + n1` and `s + n1..s + n1 + n2` into `s..s + n1 + n2`, favoring the right run
// iff `invert`.
//
// Return the number of elements in `s + n1..s + n1 + n2` involved in the merge.
unsafe fn local_merge<T, F: Less<T>>(
    buf: &mut Buffer<T>,
    s: *mut T,
    n1: usize,
    n2: usize,
    invert: bool,
    less: &mut F,
) -> usize {
    if invert {
        if less(&*s.add(n1 - 1), &*s.add(n1)) {
            return 0;
        }

        let rad = block_swap_length(s, n1, s.add(n1), n2 - 1, &mut |x, y| !less(y, x));
        buf.begin_merge(s.add(n1 - rad), rad);

        if rad > (n1 - rad) / RATIO_BIN_MERGE {
            merge_left(s.add(n1), rad, s, n1 - rad, s, less);
        } else {
            binary_merge_left(s, n1 - rad, s.add(n1), rad, s, &mut |x, y| !less(y, x));
        }

        merge_right(s.add(n1 + rad), n2 - rad, buf.start, rad, s.add(n1), less)
    } else {
        if !less(&*s.add(n1), &*s.add(n1 - 1)) {
            return 0;
        }

        let rad = block_swap_length(s, n1, s.add(n1), n2 - 1, less);
        buf.begin_merge(s.add(n1 - rad), rad);

        if rad > (n1 - rad) / RATIO_BIN_MERGE {
            merge_left(s, n1 - rad, s.add(n1), rad, s, less);
        } else {
            binary_merge_left(s, n1 - rad, s.add(n1), rad, s, less);
        }

        merge_right(buf.start, rad, s.add(n1 + rad), n2 - rad, s.add(n1), less)
    }
}

// Merge runs `s..s + n1` and `s + n1..s + n1 + n2` into `s..s + n1 + n2` with rotations, favoring
// the right run iff `invert`.
//
// Return the number of elements in `s + n1..s + n1 + n2` involved in the merge.
unsafe fn local_merge_lazy<T, F: Less<T>>(
    s: *mut T,
    n1: usize,
    n2: usize,
    invert: bool,
    less: &mut F,
) -> usize {
    let head;

    if invert {
        head = search_right(s.add(n1), n2, s.add(n1).sub(1), less);
        merge_lazy(s, n1, head, &mut |x, y| !less(y, x));
    } else {
        head = search_left(s.add(n1), n2, s.add(n1).sub(1), less);
        merge_lazy(s, n1, head, less);
    }

    head
}

/// Merge runs `s..s + n1` and `s + n1..s + n1 + n2` into `s..s + n1 + n2` using a block merge. If
/// `n2` is not divisible by `block_len`, ignore the remainder. Blocks are of length `block_len`,
/// and the block merge has access to an internal merge buffer iff `!in_place`.
///
/// Return the starting position of the suffix which needs to be merged with the remainder.
pub unsafe fn block_merge<T, F: Less<T>>(
    buf: &mut Buffer<T>,
    s: *mut T,
    n1: usize,
    n2: usize,
    block_len: usize,
    in_place: bool,
    less: &mut F,
) -> usize {
    // Sort blocks
    let keys = buf.start.add(buf.len * !in_place as usize);
    let mut blocks = Blocks::sorted_from_runs(s, keys, n1, n2, block_len, less);

    // Track position and origin of fragment
    let mut block_was_b = blocks.mid == 0;
    let mut frag = 0;

    // Merge blocks
    if in_place {
        for i in blocks.mid..blocks.count {
            if blocks.pop(i, less) ^ block_was_b {
                continue;
            }

            let pos = i * block_len;
            frag = pos + local_merge_lazy(s.add(frag), pos - frag, block_len, block_was_b, less);
            block_was_b ^= true;
        }
    } else {
        for i in blocks.mid..blocks.count {
            if blocks.pop(i, less) ^ block_was_b {
                continue;
            }

            let pos = i * block_len;
            frag = pos + local_merge(buf, s.add(frag), pos - frag, block_len, block_was_b, less);
            block_was_b ^= true;
        }
    }

    frag
}
