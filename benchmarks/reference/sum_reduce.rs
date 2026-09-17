fn compute_sum_reduce(n: usize) -> f64 {
    let mut a: Vec<f64> = Vec::with_capacity(n);
    for _ in 0..n {
        a.push(1.5);
    }
    let mut sum = 0.0;
    for idx in 0..n {
        sum += a[idx];
    }
    sum
}

fn main() {
    let t0 = std::time::Instant::now();
    let res = compute_sum_reduce(100000);
    let elapsed = t0.elapsed();
    println!("RUST_IN_PROCESS_US: {}, res: {}", elapsed.as_micros(), res);
}
