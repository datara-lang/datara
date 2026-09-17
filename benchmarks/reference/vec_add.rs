fn compute_vec_add(n: usize, iters: usize) -> f64 {
    let mut a: Vec<f64> = Vec::with_capacity(n);
    let mut b: Vec<f64> = Vec::with_capacity(n);
    let mut c: Vec<f64> = vec![0.0; n];
    for _ in 0..n {
        a.push(1.5);
        b.push(2.5);
    }
    for _ in 0..iters {
        for idx in 0..n {
            c[idx] = a[idx] + b[idx];
        }
    }
    c[n - 1]
}

fn main() {
    let t0 = std::time::Instant::now();
    let res = compute_vec_add(100000, 50);
    let elapsed = t0.elapsed();
    println!("RUST_IN_PROCESS_US: {}, res: {}", elapsed.as_micros(), res);
}
