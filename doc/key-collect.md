Visit [this page](https://sortingalgos.miraheze.org/wiki/Block_Merge_Sort#Key_Collection) to understand existing key collection techniques.

## Method

The method described here aims to collect `u = O(sqrt n)` keys in `n lg u + O(n)` comparisons and `O(n log n)` moves.

### Key collection from a sorted run with an existing buffer

Given a sorted run of `n` elements and a buffer of `k` elements, we can obtain a merged run `S` in `O(n + k)` comparisons. We can extract all distinct values from `S` in `O(n + k)` comparisons by scanning adjacent pairs of elements. As this step requires only sequential access to adjacent pairs of elements, we can avoid actually constructing `S` &mdash; simply follow the comparisons done in a merge and compare the two latest "merged" items to identify unique keys. To illustrate:

> 1. Find the first merged item
> 2. Find the second merged item
> 3. Compare the second merged item with the previous to see if it is a new key
> 4. Find the third merged item
> 5. Compare the third merge item with the previous
> 6. ...

### Algorithm

_Key collection from an unsorted sample_ can be modified to use the above algorithm for buffer comparisons.

Assume we have a buffer of `k` elements (items to the left have already been compared against the buffer).

```
____XX_____________________________________________________
```

We further assume that we have constructed runs of size `t` where `t <= 2k < 2t`, with a maximum of two undersized runs adjacent to the buffer.

```
..: XX .: ..:: ..:: ..:: ..:: ..:: ..:: ..:: ..:: ..:: ..::
```

We sequentially apply the key collection method on the runs until the buffer reaches `k' >= t` elements. If we collect `m <= t` keys from the last run before reaching at least `t` keys, then `k' < t + m <= 2t`.

Comparing each run of `t` elements costs `O(t + k) = O(k)` comparisons, so the amortized cost of checking one element against the buffer is `O(k) / t = O(1)` comparisons. We also do not consider these elements again, so it is easy to see that we spend `O(n)` of these comparisons for the entire routine.

```
..: .: ..: ..:: .:: XXXX ..:: ..:: ..:: ..:: ..:: ..:: ..::
    └── compared ─┘
```

Extracting any number of elements from one sorted run yields another sorted run. If we scanned `r` runs, we are left with at most `r` runs of arbitrary size after extracting keys. We scan these runs by comparing adjacent elements to identify boundaries. For each run, we perform a merge with an existing fragment on the left using the buffer (which is large enough with `t` items) in order to restore runs of size `t`. The combined cost of scanning and merging is `O(rt)` comparisons and moves which is linear with the number of items we compared against the buffer.

```
...: ..:: ..:: .:: XXXX ..:: ..:: ..:: ..:: ..:: ..:: ..::
└─── restored ───┘
```

Now that we have `k' >= t` buffer elements, we create runs of size `t' = 2t` by merging with the buffer.

```
.....::: ...:::: XXXX ..:: ....:::: ....:::: ....::::
```

We can proceed to the next level as `t' <= 2k' < 2t'` and there are at most two undersized fragments. The buffer is scrambled after merging but we can sort it such that the cost of sorting the buffer across all levels is `O(n)`.

## Variants

### Block merging for cleanup

Rather than scanning the runs to re-identify run boundaries, we can interleave extracting and restoring by merging immediately after extraction: use `O(sqrt t)` keys from the buffer to perform a block merge and sort the used keys in `O(t)` operations. This method can reduce comparisons but has the overhead of block merging and sorting the buffer which is unacceptable for small `t`.

```
.: XX ..:: ..:: ..:: ..::

.: .:: XXX ..:: ..:: ..::
└───┘  merge

..:: : XXX ..:: ..:: ..::

..:: : ..:: ..:: ..: XXXX
     └───┘└───┘└───┘ merge *

..:: ..:: ..:: ..:: XXXX
```

<sup>\*We know that we did not take items from the first two runs, so their lengths are known. </sup>

### Cleanup after all keys are found

The routine can be de-interleaved into two big phases: collection and run snapping. The collection phase is the same but without restoration and the merge passes act only on runs not yet compared to the buffer. After collecting all keys, the buffer is shifted out and the run snapping phase begins. In this phase, one scan pass recovers the lengths of all compared runs after extraction. The origin of any run is the value `t` at the time of comparing the run. The scanned runs are sorted by origin, so we restore run sizes appropriately if we accurately identify the level a run is from.

```
X .: .: .: .: .: .: .: .: .: .: .: .: .: .: .: .: .: .: .: .: .: .: .: .: .: .: .: .:

.: :  .:: ..:: .:: ....::: ....:::: ..::: .......::::::: XXXXXXXXXX
└ 2 ┘ └──── 4 ───┘ └───────── 8 ────────┘ └──── 16 ────┘

.: : .:: ..:: .:: ....::: ....:::: ..::: .......:::::::

.: : .:: ..:: .:: ....::: ....:::: ..: .......:::::::::     snap t = 16
    
.: : .:: ..:: .:: .. .....::: ...::::: .......:::::::::     snap t = 8

.: : ..:: ..:: ..:: .....::: ...::::: .......:::::::::      snap t = 4

. :: ..:: ..:: ..:: .....::: ...::::: .......:::::::::      snap t = 2
```

The [Rust implementation](../rs) uses this variant.
