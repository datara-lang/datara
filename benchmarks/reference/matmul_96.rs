fn compute_matmul(n: usize) -> f64 {
    let total = n * n;
    let mut a: Vec<f64> = Vec::with_capacity(total);
    let mut b: Vec<f64> = Vec::with_capacity(total);
    let mut c: Vec<f64> = Vec::with_capacity(total);
    for _ in 0..total {
        a.push(1.0);
        b.push(2.0);
        c.push(0.0);
    }
    for row in 0..n {
        for col in 0..n {
            let mut sum = 0.0;
            for k in 0..n {
                sum += a[row * n + k] * b[k * n + col];
            }
            c[row * n + col] = sum;
        }
    }
    c[total - 1]
}

fn main() {
    let t0 = std::time::Instant::now();
    let res = compute_matmul(96);
    let elapsed = t0.elapsed();
    println!("RUST_IN_PROCESS_US: {}, res: {}", elapsed.as_micros(), res);
}
