# Standard Library Reference (1.5.2)

## stdlib.testing
Test framework with pass/fail counting.
```datara
use stdlib.testing.testing_new
use stdlib.testing.test_report
fn main() {
    val t = testing_new()
    t.assert_eq_int(2 + 2, 4, "arith")
    t.assert_true(1 < 2, "lt")
    val fails = test_report(t)
}
```

## stdlib.math.stats
Statistics on List<Float>.
```datara
use stdlib.math.stats.stats_mean
use stdlib.math.stats.stats_stddev
fn main() {
    val d = [1.0, 2.0, 3.0, 4.0, 5.0]
    out float_to_str(stats_mean(d))
    out float_to_str(stats_stddev(d))
}
```

## stdlib.math.random
Deterministic xorshift64* PRNG with explicit seed.
```datara
use stdlib.math.random.random_seed
use stdlib.math.random.random_range
fn main() {
    random_seed(42)
    out int_to_str(random_range(1, 100))
}
```

## stdlib.io.csv
CSV parser/writer.
```datara
use stdlib.io.csv.parse_csv
use stdlib.io.csv.csv_row
fn main() {
    val rows = parse_csv("a,b,c\n1,2,3")
    out int_to_str(rows.len())
}
```

## stdlib.collections
Deque, Set, PriorityQueue — double-ended queue, hash-free set, max-heap.
See tests in stdlib_tests/tests/test_collections.dtr (16 checks).

## stdlib.result
Outcome<T> + Maybe<T> — deterministic error handling (see §11 of manual).

## stdlib.web.ssr
Escaped-by-default HTML rendering. See test_web_ssr.rs.

## stdlib.text
Codepoint-aware string operations. See stdlib/text/text.dtr.
