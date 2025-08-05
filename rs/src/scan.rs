use crate::{
    dust::{insert_sort, MIN_RUN},
    util::{advance, ptr_sub, reverse, Less},
};

/// Return the length of the longest non-descending prefix of `s..s + n`.
pub unsafe fn next_non_desc_run<T, F: Less<T>>(s: *mut T, n: usize, less: &mut F) -> usize {
    (1..n)
        .find(|&i| less(&*s.add(i), &*s.add(i - 1)))
        .unwrap_or(n)
}

/// Construct the next longest run starting at `s` with max length `n`.
///
/// Return the length of the run.
pub unsafe fn next_sorted_run<T, F: Less<T>>(s: *mut T, n: usize, less: &mut F) -> usize {
    // Scan for initial non-descending run
    let mut i = next_non_desc_run(s, n, less);

    if i == n || i > 1 && less(&*s, &*s.add(i - 1)) {
        return i;
    }

    let mut l = s.add(i);
    reverse(s, s.add(i));

    // Flip equal segments until we reach an ascending pair
    loop {
        i += 1;

        if i == n {
            break;
        }

        if less(&*s.add(i), &*s.add(i - 1)) {
            reverse(l, s.add(i));
            l = s.add(i);
        } else if less(&*s.add(i - 1), &*s.add(i)) {
            break;
        }
    }

    reverse(l, s.add(i));
    reverse(s, s.add(i));
    i
}

/// Build runs of the minimum starting length on `s..s + n` assuming the first `i` elements are done
/// already. Only the last/rightmost run may be less than the minimum length.
pub unsafe fn build_runs<T, F: Less<T>>(mut s: *mut T, mut i: *mut T, mut n: usize, less: &mut F) {
    i = <*mut T>::max(i, s.add(1));

    while n > 0 {
        let offset = ptr_sub(i, s);
        (s, n) = advance(s, n, offset.next_multiple_of(MIN_RUN) - MIN_RUN);

        let len = usize::min(n, MIN_RUN);
        insert_sort(s, usize::max(1, offset % MIN_RUN), len, less);

        (s, n) = advance(s, n, len);
        i = s.add(next_sorted_run(s, n, less));
    }
}
