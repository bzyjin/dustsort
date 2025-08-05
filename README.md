# dustsort

**dustsort** is an adaptive block merge sort achieving smooth adaptivity to various input patterns with strong worst-case performance.

```
average     worst       best    space   stable
n log n     n log n     n       1       ✓
```

### Visualization

Preliminary visualizations are done with [ArrayV](https://github.com/Gaming32/ArrayV). Various input sizes and patterns are shown.

[![Visualization](https://i.ytimg.com/vi/nBRzPWcui6w/mqdefault.jpg)](https://www.youtube.com/watch?v=nBRzPWcui6w)

## Motivation

One hurdle that has hindered block merge sorts from practical use is the number of comparisons required in the worst case. Classic merge sort makes no more than `n lg n + O(n)` comparisons; compare to notable block merge sorts, like [GrailSort](https://github.com/Mrrl/GrailSort) which can spend `0.5 n lg n` extra comparisons on collecting keys, or [WikiSort](github.com/BonzaiThePenguin/WikiSort) which by design performs `O(n log n)` extra comparisons worth of block selections, and the difference is obvious.

In fact, one paper ([pdf](https://users.informatik.uni-halle.de/~ahyjb/sort.pdf)) also posed the following open problem (thanks to aphitorite for sharing):

>_Does there [exist] a stable in-place sorting algorithm, which needs only n log n + O(n) comparisons and only O(n log n) transports?_

The answer is **yes**. If we ignore the aforementioned key collection cost, GrailSort's block merging routine achieves these bounds; dustsort follows the same routine but introduces a key collection algorithm that needs no more than `O(n)` comparisons and `O(n)` writes. An in-depth description of the entire sorting routine, including this algorithm, is being written.

Furthermore, the Zig standard library uses WikiSort at the time of writing. GrailSort was also investigated in the past, since both are stable and in-place. Due to its adaptivity and better performance guarantees, dustsort may be a future candidate.

## Design

The overall sorting routine was designed towards the following goals:

- **Efficient**: performance without exploiting patterns is still comparable to a generic branchless merge sort.

- **Pattern-defeating**: in the worst case, the algorithm makes no more than `O(n)` extra comparisons over the average case.

- **Pattern-adaptive**: various common input patterns are exploited to reduce operations and cut down runtime.

    | Pattern               | Procedure
    | --------------------- | -
    | Non-descending runs   | Runs are sorted in linear time during small-sort phase 
    | Non-ascending runs    | Runs are stably reversed in linear time and at relatively low cost during small-sort phase
    | Roughly sorted runs   | Both comparisons and moves are reduced according to merge bounds
    | Low cardinality       | If the array has a constant number of unique items, an easy algorithm sorts in worst case `O(n log n)` operations
    | Append                | Given a sorted run with `t` appended items ending in at least `t - O(sqrt n)` low cardinality items, an easy algorithm sorts in worst case `O(n + t log t)` operations
    | Low inversions        | Comparisons and block merging levels are reduced using an unbalanced merge algorithm
    | Random                | Wasted writes from transferring run segments into buffer space are halved on average

## Performance

Benchmarks of the Rust implementation are in progress. Its performance may also be improved (especially on smaller inputs).

## Attributions

Thanks to:
- [@amari-calipso](https://github.com/amari-calipso) for inspiration (Helium Sort) and for suggesting to improve key collection
- [@aphitorite](https://github.com/aphitorite) and [Control](https://github.com/Control55) for discussions on the key collection algorithm
- [@Morwenn](https://github.com/Morwenn) for a great article on [measures of disorder](https://morwenn.github.io/algorithms,/sorting,/presortedness/2025/06/15/TSB001-amp-a-new-measure-of-presortedness.html)
