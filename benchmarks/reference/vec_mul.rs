fn compute_vec_mul(n: usize) -> f64 {
    let mut a: Vec<f64> = Vec::with_capacity(n);
    let mut b: Vec<f64> = Vec::with_capacity(n);
    let mut c: Vec<f64> = Vec::with_capacity(n);
    for _ in 0..n {
        a.push(1.25);
        b.push(4.0);
        c.push(0.0);
    }
    for idx in 0..n {
        c[idx] = a[idx] * b[idx];
    }
    c[n - 1]
}

fn main() {
    let t0 = std::time::Instant::now();
    let res = compute_vec_mul(100000);
    let elapsed = t0.elapsed();
    println!("RUST_IN_PROCESS_US: {}, res: {}", elapsed.as_micros(), res);
}
