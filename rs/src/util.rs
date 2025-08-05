use core::ptr;

/// A trait alias for comparators
pub trait Less<T>: FnMut(&T, &T) -> bool {}
impl<T, F: FnMut(&T, &T) -> bool> Less<T> for F {}

/// Represents a hole created when moving an element into stack space
pub struct Hole<T> {
    /// Pointer to the position of the hole in memory
    pub pos: *mut T,

    // Pointer to the value that should be dropped back into the hole
    src: *const T,
}

impl<T> Hole<T> {
    /// Create a new hole at position `pos` with value pointed to by `src`.
    pub const unsafe fn new(pos: *mut T, src: *const T) -> Self {
        Self { pos, src }
    }

    /// Cycle one value for merging. This reduces writes to memory.
    ///
    /// Write the value at `dst` into this hole and replace it with the value at `src`, moving the
    /// hole to `src`.
    #[inline(always)]
    pub unsafe fn cycle(&mut self, src: *mut T, dst: *mut T) {
        self.pos.write(dst.read());
        dst.write(src.read());
        self.pos = src;
    }
}

impl<T> Drop for Hole<T> {
    fn drop(&mut self) {
        unsafe {
            self.pos.write(self.src.read());
        }
    }
}

/// Return the number of elements `r` is offset by from `l`, assuming `r >= l`.
pub unsafe fn ptr_sub<T>(r: *const T, l: *const T) -> usize {
    core::hint::assert_unchecked(l <= r);
    r.offset_from(l) as usize
}

/// Given a range `s..s + n`, return the corresponding values to represent `s + cnt..s + n`.
pub unsafe fn advance<T>(s: *mut T, n: usize, cnt: usize) -> (*mut T, usize) {
    (s.add(cnt), n - cnt)
}

/// Swap two regions starting with `a` and `b` and ending before `a + cnt` and `b + cnt`.
///
/// The relative order of the region starting at `a` is not preserved.
pub unsafe fn cycle_swap<T>(a: *mut T, b: *mut T, cnt: usize) {
    // Hint that regions don't overlap
    core::hint::assert_unchecked(a.add(cnt) <= b || b.add(cnt) <= a);

    let tmp = a.read();
    a.write(b.read());

    for i in 1..cnt {
        b.add(i - 1).write(a.add(i).read());
        a.add(i).write(b.add(i).read());
    }

    b.add(cnt - 1).write(tmp);
}

/// Return `b` if `is_b` or `a` otherwise.
#[inline(always)]
pub fn conditional<T: Copy>(a: T, b: T, is_b: bool) -> T {
    [a, b][is_b as usize]
}

/// Shift the element at `s` to the left by `cnt` elements.
pub unsafe fn insert_left<T>(s: *mut T, cnt: usize) {
    let tmp = s.read();
    ptr::copy(s.sub(cnt), s.add(1).sub(cnt), cnt);
    s.sub(cnt).write(tmp);
}

/// Shift the element at `s` to the right by `cnt` elements.
pub unsafe fn insert_right<T>(s: *mut T, cnt: usize) {
    let tmp = s.read();
    ptr::copy(s.add(1), s, cnt);
    s.add(cnt).write(tmp);
}

/// Reverse the region `l..r`  in-place.
#[inline(always)]
pub unsafe fn reverse<T>(mut l: *mut T, mut r: *mut T) {
    while l.add(1) < r {
        r = r.sub(1);
        ptr::swap(l, r);
        l = l.add(1);
    }
}

/// Exchange the regions `s..n1` and `s + n1..s + n1 + n2` in-place.
pub unsafe fn rotate<T>(mut s: *mut T, mut n1: usize, mut n2: usize) {
    // `slice::rotate` uses 24 elements of stack space -- not approved

    while n1 > 1 && n2 > 1 {
        if n1 > n2 {
            ptr::swap_nonoverlapping(s.add(n1 - n2), s.add(n1), n2);
            n1 -= n2;
        } else {
            ptr::swap_nonoverlapping(s, s.add(n1), n1);
            n2 -= n1;
            s = s.add(n1);
        }
    }

    if n1 == 1 {
        insert_right(s, n2);
    } else if n2 == 1 {
        insert_left(s.add(n1), n1);
    }
}

/// Return the value `i` in `0..=n` such that for all `j` in `0..i`, `f(j)` and for all `j` in
/// `i..n`, `!f(j)`. The caller guarantees `f` is partitioned in such a manner.
fn lower_bound(mut n: usize, mut f: impl FnMut(usize) -> bool) -> usize {
    let mut i = 0;

    while n > 0 {
        let h = n / 2;
        i += conditional(0, n - h, f(i + h));
        n = h;
    }

    i
}

/// Return the number of elements in the region `s..s + n` which are `less` than `val`.
pub unsafe fn search_left<T, F: Less<T>>(
    s: *const T,
    n: usize,
    val: *const T,
    less: &mut F,
) -> usize {
    lower_bound(n, |x| less(&*s.add(x), &*val))
}

/// Return the number of elements in the region `s..s + n` which `val` is not `less` than.
pub unsafe fn search_right<T, F: Less<T>>(
    s: *const T,
    n: usize,
    val: *const T,
    less: &mut F,
) -> usize {
    lower_bound(n, |x| !less(&*val, &*s.add(x)))
}

/// Return the largest number of elements `e` such that the leftmost `e` elements in the region
/// `s2..s2 + n2` are `less` than the rightmost `e` elements in the region `s1..s1 + n1`.
pub unsafe fn block_swap_length<T, F: Less<T>>(
    s1: *const T,
    n1: usize,
    s2: *const T,
    n2: usize,
    less: &mut F,
) -> usize {
    lower_bound(usize::min(n1, n2), |i| {
        less(&*s2.add(i), &*s1.add(n1 - i - 1))
    })
}
