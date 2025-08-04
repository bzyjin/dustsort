use crate::{
    blocks::block_merge,
    buffer::Buffer,
    merge::{merge, merge_in_place},
    scan::{build_runs, next_non_desc_run, next_sorted_run},
    util::{ptr_sub, Hole, Less},
};

/// Create runs of this size at the small-sort level.
pub const MIN_RUN: usize = 32;

/// For two runs of size `n1, n2` where `n1 | n2 < [value]`, prefer simple lazy stable merging over
/// the advanced version.
pub const MIN_FAST_LAZY: usize = 512;

/// For two runs of size `n1, n2` where the smaller run size `n1` satisfies `n1 <= n2 / [ratio]`,
/// prefer binary merging over branchless merging.
pub const RATIO_BIN_MERGE: usize = 8;

// Immediately switch to insertion sort if the array is smaller than this.
const MIN_SCAN: usize = 8;

// Avoid the overhead of block merge sort on arrays smaller than this.
const MIN_MERGE_SORT: usize = 64;

// For arrays smaller than this, use naive key collection. Although extremely rare, linear key
// collect might make `O(n)` writes and more than `4 * n` comparisons, which isn't much better than
// pure binary searches.
const MIN_OPT_FIND_KEYS: usize = 4096;

// Use a special strategy on arrays with less than this many comparatively unequal elements.
const MIN_DISTINCT: usize = 12;

// Use a special strategy on arrays which end in no more than this many unsorted blocks. This is
// still applicable with arbitrarily many further elements, as long as those elements have no more
// than `MIN_DISTINCT` comparatively unequal elements.
const MAX_APPEND_BLOCKS: usize = 3;

// Return the desired block length to sort `n` elements.
fn array_block_length(n: usize) -> usize {
    let k = 1 << ((n.ilog2() + 1) / 2);
    k << (k < n / k) as usize
}

// Return the desired block length for a buffer of size `buf_len`.
fn buffer_block_length(buf_len: usize) -> usize {
    2 << ((buf_len + 2) / 3).ilog2()
}

/// Sort `s..s + n` with insertion sort, assuming the first `i` elements are sorted.
pub unsafe fn insert_sort<T, F: Less<T>>(s: *mut T, i: usize, n: usize, less: &mut F) {
    for i in i..n {
        let tmp = core::mem::ManuallyDrop::new(s.add(i).read());
        let mut hole = Hole::new(s.add(i), &*tmp);

        while hole.pos > s.add(1) && less(&tmp, &*hole.pos.sub(2)) {
            hole.pos.write(hole.pos.sub(1).read());
            hole.pos.sub(1).write(hole.pos.sub(2).read());
            hole.pos = hole.pos.sub(2);
        }

        if hole.pos > s {
            // Compare first to ensure identical copies
            let odd = less(&tmp, &*hole.pos.sub(1));
            hole.pos.write(hole.pos.sub(1).read());
            hole.pos = hole.pos.sub(odd as usize);
        }
    }
}

// Sort `s..n` with a rotation-based merge sort, assuming the first `head` elements were already
// sorted before runs of size `run` were created.
unsafe fn merge_sort_in_place<T, F: Less<T>>(
    s: *mut T,
    head: usize,
    n: usize,
    mut run: usize,
    less: &mut F,
) {
    while run < n {
        let mut l = head - head % (2 * run);

        while l + 2 * run <= n {
            merge_in_place(s.add(l), run, run, less);
            l += 2 * run;
        }

        if l + run < n {
            merge_in_place(s.add(l), run, n - (l + run), less);
        }

        run *= 2;
    }
}

// Special sorting routine: use only rotation-based merging to sort in worst case `O(n log n)` time.
// This avoids collecting an internal buffer.
unsafe fn sort_special<T, F: Less<T>>(s: *mut T, n: usize, head: usize, tail: usize, less: &mut F) {
    build_runs(s, s.add(head), n - tail, less);
    merge_sort_in_place(s, head, n - tail, MIN_RUN, less);

    if tail > 0 {
        build_runs(s, s.add(n - tail), n, less);
        merge_sort_in_place(s, n - tail, n, MIN_RUN, less);
    }
}

// Sort `s..buf.start` with block merge sort given `buf` as an internal buffer, assuming runs of
// length `run` are already built on `0..tail_start`, and runs of length `MIN_RUN` are built on
// `tail_start..`.
unsafe fn block_merge_sort<T, F: Less<T>>(
    buf: &mut Buffer<T>,
    s: *mut T,
    head_run: usize,
    tail_start: usize,
    less: &mut F,
) {
    // Set up the buffer layout
    let mut block_len = buffer_block_length(buf.len);
    let keys = buf.len - block_len + 1;
    buf.len = block_len - 1;

    let mut run = MIN_RUN;
    let n = ptr_sub(buf.start, s);

    // Block merging with merge buffer
    while run < n && usize::min(n, 2 * run) / block_len <= keys {
        let mut l = tail_start * (run < head_run) as usize;

        while l + 2 * run <= n {
            if !merge(buf, s.add(l), run, run, less) {
                block_merge(buf, s.add(l), run, run, block_len, false, less);
            }

            l += 2 * run;
        }

        if l + run < n && !merge(buf, s.add(l), run, n - (l + run), less) {
            l += block_merge(buf, s.add(l), run, n - (l + run), block_len, false, less);
            merge(buf, s.add(l), n - l - n % block_len, n % block_len, less);
        }

        run *= 2;
    }

    buf.sort(less);
    buf.len += keys;

    // Block merging without merge buffer
    while run < n {
        while usize::min(n, 2 * run) / block_len > buf.len {
            block_len *= 2;
        }

        let mut l = tail_start * (run < head_run) as usize;

        while l + 2 * run <= n {
            block_merge(buf, s.add(l), run, run, block_len, true, less);
            l += 2 * run;
        }

        if l + run + block_len < n {
            l += block_merge(buf, s.add(l), run, n - (l + run), block_len, true, less);
        }

        merge_in_place(s.add(l), n - l - n % block_len, n % block_len, less);

        run *= 2;
    }
}

/// Sort `s..s + n` with dustsort.
pub unsafe fn sort<T, F: Less<T>>(s: *mut T, n: usize, less: &mut F) {
    if n < MIN_SCAN {
        return insert_sort(s, 1, n, less);
    }

    let mut head = next_sorted_run(s, n, less);
    head += next_non_desc_run(s.add(head - 1), n - (head - 1), less) - 1;

    if head == n {
        return;
    }

    if n < MIN_MERGE_SORT {
        return insert_sort(s, head, n, less);
    }

    let block_len = array_block_length(n + 1);

    // For small appended tails, sort immediately with rotations
    if head + block_len * MAX_APPEND_BLOCKS >= n {
        return sort_special(s, n, head, 0, less);
    }

    let mut buf = Buffer {
        start: s.add(n),
        len: 0,
        unsorted: 0,
    };

    buf.binary_find_keys(s.add(head), s.add(n), 12, less);

    // For many similar items excluding head, sort immediately with rotations
    if buf.len < MIN_DISTINCT {
        buf.shift(s.add(n - buf.len));
        return sort_special(s, n, head, n - head, less);
    }

    // Combine both cases above
    if buf.start <= s.add(head + block_len * MAX_APPEND_BLOCKS) {
        let tail = ptr_sub(s.add(n), buf.start);
        buf.shift(s.add(n - buf.len));
        return sort_special(s, n, head, tail, less);
    }

    // Ideal number of buffer elements to guarantee all merges are buffered
    let ideal = block_len + (n + 1) / block_len - 2;

    // See comment on [`MIN_OPT_FIND_KEYS`]
    if n < MIN_OPT_FIND_KEYS {
        buf.binary_find_keys(s.add(head), buf.start, ideal, less);

        if buf.len < ideal {
            let tmp_len = buf.len;
            buf.batch_find_keys(s, s.add(head), ideal, less);
            head -= buf.len - tmp_len;
        }

        buf.shift(s.add(n - buf.len));
        build_runs(s, s.add(head), n - buf.len, less);
        block_merge_sort(&mut buf, s, MIN_RUN, 0, less);
        merge_in_place(s, n - buf.len, buf.len, less);

        return;
    }

    let mut l = ptr_sub(buf.start, s);
    let mut r = l + buf.len;
    let mut run = MIN_RUN;

    build_runs(s, s.add(head), l, less);

    // Collect distinct keys
    while l > 0 {
        let len = (l - 1) % run + 1;
        buf.batch_find_keys(s.add(l - len), s.add(l), ideal, less);
        l -= len;

        if buf.len == ideal {
            break;
        }

        if buf.len >= run {
            // Merge pass
            for i in (2 * run..(l + 1)).step_by(2 * run) {
                merge(&mut buf, s.add(i - 2 * run), run, run, less);
            }

            buf.sort(less);
            run *= 2;
        }
    }

    // Align buffer to the right
    buf.shift(s.add(n - buf.len));

    let tail_start = usize::max(l + run, head) / run * run - run;
    l = tail_start;
    r -= buf.len;

    let mut frag = 0;
    let mut prev = run;

    // Snap blocks to powers of two
    while l < r {
        let cur = next_non_desc_run(s.add(l), r - l, less);

        if run > MIN_RUN && cur <= run / 2 && prev + cur <= run {
            run /= 2;
        }

        frag %= run;
        merge(&mut buf, s.add(l - frag), frag, cur.min(run - frag), less);

        l += cur;
        frag += cur;
        prev = cur;
    }

    buf.sort(less);
    build_runs(s, s.add(r), n - buf.len, less);

    // Now we have runs in non-ascending powers of two e.g. `256 128 128 64 64 64 32 ...`
    block_merge_sort(&mut buf, s, run, tail_start, less);
    merge_in_place(s, n - buf.len, buf.len, less);
}
