use core::mem::MaybeUninit;
use core::ptr;

use crate::{
    buffer::Buffer,
    dust::{MIN_FAST_LAZY, RATIO_BIN_MERGE},
    util::{
        advance, block_swap_length, conditional, cycle_swap, rotate, search_left, search_right,
        Hole, Less,
    },
};

/// Swap two regions starting with `a` and `b` and ending before `a + cnt` and `b + cnt`
///
/// The relative order of the region starting at `a` is not preserved.
unsafe fn end_merge<T>(dst: *mut T, src: *mut T, cnt: usize) {
    if dst != src {
        cycle_swap(dst, src, cnt);
    }
}

/// Merge runs `s1..s1 + n1` and `s2..s2 + n2` into `dst..dst + n1 + n2` using a classic rightwards
/// merge.
///
/// Return the number of elements merged in the loop.
pub unsafe fn merge_right<T, F: Less<T>>(
    s1: *mut T,
    n1: usize,
    s2: *mut T,
    n2: usize,
    mut dst: *mut T,
    less: &mut F,
) -> usize {
    let mut i1 = 0;
    let mut i2 = 0;

    let mut tmp = MaybeUninit::uninit();
    let mut hole = Hole::new(tmp.as_mut_ptr(), tmp.as_ptr());

    while i1 < n1 && i2 < n2 {
        let is_2 = less(&*s2.add(i2), &*s1.add(i1));
        hole.cycle(conditional(s1.add(i1), s2.add(i2), is_2), dst);

        i1 += !is_2 as usize;
        i2 += is_2 as usize;
        dst = dst.add(1);
    }

    drop(hole);
    let src = conditional(s1.add(i1), s2.add(i2), i2 < n2);
    end_merge(dst, src, (n1 - i1) + (n2 - i2));

    i1 + i2
}

/// Merge runs `s1..s1 + n1` and `s2..s2 + n2` into `dst..dst + n1 + n2` using a classic leftwards
/// merge.
pub unsafe fn merge_left<T, F: Less<T>>(
    s1: *mut T,
    mut n1: usize,
    s2: *mut T,
    mut n2: usize,
    dst: *mut T,
    less: &mut F,
) {
    let mut dst_rev = dst.add(n1 + n2);

    let mut tmp = MaybeUninit::uninit();
    let mut hole = Hole::new(tmp.as_mut_ptr(), tmp.as_ptr());

    while n1 > 0 && n2 > 0 {
        dst_rev = dst_rev.sub(1);

        let is_1 = less(&*s2.add(n2 - 1), &*s1.add(n1 - 1));
        n1 -= is_1 as usize;
        n2 -= !is_1 as usize;

        hole.cycle(conditional(s2.add(n2), s1.add(n1), is_1), dst_rev);
    }

    drop(hole);
    end_merge(dst, conditional(s1, s2, n2 > 0), n1 | n2);
}

/// Merge runs `s1..s1 + n1` and `s2..s2 + n2` into `dst..dst + n1 + n2` using a binary rightwards
/// merge.
///
/// Return the number of elements merged in the loop.
pub unsafe fn binary_merge_right<T, F: Less<T>>(
    s1: *mut T,
    n1: usize,
    s2: *mut T,
    n2: usize,
    mut dst: *mut T,
    less: &mut F,
) -> usize {
    let gap = (n1 + n2 - 1) / n1;
    let mut bucket = (n2 + gap - 1) % gap;

    let mut i1 = 0;
    let mut i2 = 0;
    let mut f2 = search_left(s2, bucket + 1, s1, less);

    let mut tmp = MaybeUninit::uninit();
    let mut hole = Hole::new(tmp.as_mut_ptr(), tmp.as_ptr());

    while i2 < n2 {
        if i2 < f2 {
            hole.cycle(s2.add(i2), dst);
            dst = dst.add(1);
            i2 += 1;
            continue;
        }

        if i2 > bucket {
            bucket += gap;
        } else {
            hole.cycle(s1.add(i1), dst);
            dst = dst.add(1);
            i1 += 1;

            if i1 == n1 {
                break;
            }
        }

        f2 = bucket + 1;

        if !less(&*s2.add(bucket), &*s1.add(i1)) {
            f2 = i2 + search_left(s2.add(i2), bucket - i2, s1.add(i1), less);
        }
    }

    drop(hole);
    let src = conditional(s1.add(i1), s2.add(i2), i2 < n2);
    end_merge(dst, src, (n1 - i1) + (n2 - i2));

    i1 + i2
}

/// Merge runs `s1..s1 + n1` and `s2..s2 + n2` into `dst..dst + n1 + n2` using a binary leftwards
/// merge.
pub unsafe fn binary_merge_left<T, F: Less<T>>(
    s1: *mut T,
    mut n1: usize,
    s2: *mut T,
    mut n2: usize,
    dst: *mut T,
    less: &mut F,
) {
    let gap = (n1 + n2 - 1) / n2;
    let mut bucket = (n1 - 1) / gap * gap;

    let mut dst_rev = dst.add(n1 + n2);
    let mut f1 = bucket + search_right(s1.add(bucket), n1 - bucket, s2.add(n2 - 1), less);

    let mut tmp = MaybeUninit::uninit();
    let mut hole = Hole::new(tmp.as_mut_ptr(), tmp.as_ptr());

    while n1 > 0 {
        if n1 > f1 {
            dst_rev = dst_rev.sub(1);
            n1 -= 1;
            hole.cycle(s1.add(n1), dst_rev);
            continue;
        }

        if n1 == bucket {
            bucket -= gap;
        } else {
            dst_rev = dst_rev.sub(1);
            n2 -= 1;
            hole.cycle(s2.add(n2), dst_rev);

            if n2 == 0 {
                break;
            }
        }

        f1 = bucket;

        if !less(&*s2.add(n2 - 1), &*s1.add(bucket)) {
            f1 += 1 + search_right(s1.add(bucket + 1), n1 - bucket - 1, s2.add(n2 - 1), less);
        }
    }

    drop(hole);
    end_merge(dst, conditional(s1, s2, n2 > 0), n1 | n2);
}

/// Try to merge runs `s..s + n1` and `s + n1..s + n1 + n2` using an adaptive merge.
///
/// Return `false` if the merge could not be completed.
#[inline(never)]
pub unsafe fn merge<T, F: Less<T>>(
    buf: &mut Buffer<T>,
    s: *mut T,
    n1: usize,
    n2: usize,
    less: &mut F,
) -> bool {
    if n1 == 0 || n2 == 0 || !less(&*s.add(n1), &*s.add(n1 - 1)) {
        return true;
    }

    if less(&*s.add(n1 + n2 - 1), &*s) {
        rotate(s, n1, n2);
        return true;
    }

    let rad = block_swap_length(s, n1, s.add(n1), n2, less);

    if rad > buf.len {
        if usize::max(n1, n2) - rad > buf.len {
            return false;
        }

        // Split case into two merges
        ptr::swap_nonoverlapping(s.add(n1 - rad), s.add(n1), rad);
        return merge(buf, s, n1 - rad, rad, less) && merge(buf, s.add(n1), rad, n2 - rad, less);
    }

    buf.begin_merge(s.add(n1 - rad), rad);

    if rad > (n1 - rad) / RATIO_BIN_MERGE {
        merge_left(s, n1 - rad, s.add(n1), rad, s, less);
    } else {
        binary_merge_left(s, n1 - rad, s.add(n1), rad, s, less);
    }

    if rad > (n2 - rad) / RATIO_BIN_MERGE {
        merge_right(buf.start, rad, s.add(n1 + rad), n2 - rad, s.add(n1), less);
    } else {
        binary_merge_right(buf.start, rad, s.add(n1 + rad), n2 - rad, s.add(n1), less);
    }

    true
}

/// Merge runs `s..s + n1` and `s + n1..s + n1 + n2` into `s..s + n1 + n2` with rotations.
pub unsafe fn merge_lazy<T, F: Less<T>>(mut s: *mut T, mut n1: usize, mut n2: usize, less: &mut F) {
    if n2 <= n1 {
        while n2 > 0 {
            let next_1 = search_right(s, n1, s.add(n1 + n2 - 1), less);

            rotate(s.add(next_1), n1 - next_1, n2);
            n1 = next_1;

            if n1 == 0 {
                break;
            }

            n2 = search_left(s.add(n1), n2 - 1, s.add(n1 - 1), less);
        }
    } else {
        while n1 > 0 {
            let r_adv = search_left(s.add(n1), n2, s, less);

            rotate(s, n1, r_adv);
            (s, n2) = advance(s, n2, r_adv);

            if n2 == 0 {
                break;
            }

            (s, n1) = advance(s, n1, 1 + search_right(s.add(1), n1 - 1, s.add(n1), less));
        }
    }
}

/// Merge runs `s..s + n1` and `s + n1..s + n1 + n2` using a rotation-based merge algorithm that can
/// have better performance than [`merge_lazy`] when data moves are expensive.
pub unsafe fn merge_in_place<T, F: Less<T>>(
    mut s: *mut T,
    mut n1: usize,
    mut n2: usize,
    less: &mut F,
) {
    if n1 == 0 || n2 == 0 || !less(&*s.add(n1), &*s.add(n1 - 1)) {
        return;
    }

    if n1 | n2 < MIN_FAST_LAZY {
        return merge_lazy(s, n1, n2, less);
    }

    // Trim right run; necessary for this algorithm to work on special sort
    n2 = search_left(s.add(n1), n2, s.add(n1 - 1), less);

    // Use as a milestone for checking the merge ratio
    let mut log_step = n2;

    // Termination: n1' + n2' == max(n1, n2)
    loop {
        if n1 <= log_step || n2 <= log_step {
            let min = usize::min(n1, n2);

            if min == 0 {
                return;
            }

            // Break at ratio `sqrt n : n`
            if min * 2 + 1 < (n1 + n2) / min {
                return merge_lazy(s, n1, n2, less);
            }

            log_step = min.next_power_of_two() / 2;
        }

        let rad = block_swap_length(s, n1, s.add(n1), n2, less);
        ptr::swap_nonoverlapping(s.add(n1 - rad), s.add(n1), rad);

        // Reduce case to (1) left or (2) right merge
        if n2 < n1 {
            merge_lazy(s.add(n1), rad, n2 - rad, less);
            (n1, n2) = (n1 - rad, rad);
        } else {
            merge_lazy(s, n1 - rad, rad, less);
            s = s.add(n1);
            (n1, n2) = (rad, n2 - rad);
        }
    }
}
