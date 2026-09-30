fn fib(n: i64) -> i64 {
    if n < 2 { n } else { fib(n - 1) + fib(n - 2) }
}

fn main() {
    let mut total: i64 = 0;
    let mut i = 0;
    while i < 10 {
        total += fib(30);
        i += 1;
    }
    println!("{}", total);
}
