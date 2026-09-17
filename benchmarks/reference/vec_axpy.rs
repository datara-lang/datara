fn compute_axpy(n: usize, alpha: f64) -> f64 {
    let mut x: Vec<f64> = Vec::with_capacity(n);
    let mut y: Vec<f64> = Vec::with_capacity(n);
    for _ in 0..n {
        x.push(2.0);
        y.push(10.0);
    }
    for idx in 0..n {
        y[idx] = alpha * x[idx] + y[idx];
    }
    y[n - 1]
}

fn main() {
    let t0 = std::time::Instant::now();
    let res = compute_axpy(100000, 3.5);
    let elapsed = t0.elapsed();
    println!("RUST_IN_PROCESS_US: {}, res: {}", elapsed.as_micros(), res);
}
