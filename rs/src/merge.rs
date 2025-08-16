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

    if dst != src {
        cycle_swap(dst, src, (n1 - i1) + (n2 - i2));
    }

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

    let src = conditional(s1, s2, n2 > 0);

    if dst != src {
        cycle_swap(dst, src, n1 | n2);
    }
}

/// Merge runs `s1..s1 + n1` and `s2..s2 + n2` into `dst..dst + n1 + n2` using a rightwards merge
/// with exponential search.
pub unsafe fn exponential_merge_right<T, F: Less<T>>(
    s1: *mut T,
    n1: usize,
    s2: *mut T,
    n2: usize,
    mut dst: *mut T,
    less: &mut F,
) {
    let mut i1 = 0;
    let mut i2 = 0;

    let mut tmp = MaybeUninit::uninit();
    let mut hole = Hole::new(tmp.as_mut_ptr(), tmp.as_ptr());

    while i1 < n1 && i2 < n2 {
        let mut d = 0;

        while i2 + d < n2 && less(&*s2.add(i2 + d), &*s1.add(i1)) {
            d = d * 2 + 1;
        }

        let mut r = i2 + (d + 1) / 2;
        r += search_left(s2.add(r), usize::min(i2 + d, n2) - r, s1.add(i1), less);

        while i2 < r {
            hole.cycle(s2.add(i2), dst);
            dst = dst.add(1);
            i2 += 1;
        }

        hole.cycle(s1.add(i1), dst);
        dst = dst.add(1);
        i1 += 1;
    }

    drop(hole);

    if i1 < n1 {
        cycle_swap(dst, s1.add(i1), n1 - i1);
    }
}

/// Merge runs `s1..s1 + n1` and `s2..s2 + n2` into `dst..dst + n1 + n2` using a leftwards merge
/// with exponential search.
pub unsafe fn exponential_merge_left<T, F: Less<T>>(
    s1: *mut T,
    mut n1: usize,
    s2: *mut T,
    mut n2: usize,
    dst: *mut T,
    less: &mut F,
) {
    let mut tmp = MaybeUninit::uninit();
    let mut hole = Hole::new(tmp.as_mut_ptr(), tmp.as_ptr());
    let mut dst_rev = dst.add(n1 + n2);

    while n1 > 0 && n2 > 0 {
        let mut d = 1;

        while d <= n1 && less(&*s2.add(n2 - 1), &*s1.add(n1 - d)) {
            d *= 2;
        }

        let mut l = n1.saturating_sub(d - 1);
        l += search_right(s1.add(l), n1 - d / 2 - l, s2.add(n2 - 1), less);

        while n1 > l {
            dst_rev = dst_rev.sub(1);
            n1 -= 1;
            hole.cycle(s1.add(n1), dst_rev);
        }

        dst_rev = dst_rev.sub(1);
        n2 -= 1;
        hole.cycle(s2.add(n2), dst_rev);
    }

    drop(hole);

    if n2 > 0 {
        cycle_swap(dst, s2, n2);
    }
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
        exponential_merge_left(s, n1 - rad, s.add(n1), rad, s, less);
    }

    if rad > (n2 - rad) / RATIO_BIN_MERGE {
        merge_right(buf.start, rad, s.add(n1 + rad), n2 - rad, s.add(n1), less);
    } else {
        exponential_merge_right(buf.start, rad, s.add(n1 + rad), n2 - rad, s.add(n1), less);
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

            n1 -= rad;
            n2 = rad;
        } else {
            merge_lazy(s, n1 - rad, rad, less);

            s = s.add(n1);
            n1 = rad;
            n2 -= rad;
        }
    }
}
