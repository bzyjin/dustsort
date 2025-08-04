#![no_std]

use core::cmp::Ordering;

mod blocks;
mod buffer;
mod dust;
mod merge;
mod scan;
mod util;

/// Sort `v`.
#[inline(always)]
pub fn sort<T: Ord>(v: &mut [T]) {
    sort_common(v, &mut T::lt);
}

/// Sort `v` with a comparator `compare`.
#[inline(always)]
pub fn sort_by<T, F: FnMut(&T, &T) -> Ordering>(v: &mut [T], mut compare: F) {
    sort_common(v, &mut |x, y| compare(x, y) == Ordering::Less);
}

/// Sort `v` with a key extraction function `f`.
#[inline(always)]
pub fn sort_by_key<T, K: Ord, F: FnMut(&T) -> K>(v: &mut [T], mut f: F) {
    sort_common(v, &mut |x, y| f(x).lt(&f(y)));
}

#[inline(always)]
fn sort_common<T, F: FnMut(&T, &T) -> bool>(v: &mut [T], less: &mut F) {
    // Ignore ZSTs
    if core::mem::size_of::<T>() == 0 {
        return;
    }

    unsafe {
        dust::sort(v.as_mut_ptr(), v.len(), less);
    }
}
