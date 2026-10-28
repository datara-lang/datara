fn main() {
    let n: usize = 256;
    let mut a: Vec<i64> = vec![0; n*n];
    let mut b: Vec<i64> = vec![0; n*n];
    for i in 0..n*n {
        a[i] = (i as i64) % 7;
        b[i] = ((i as i64) * 3) % 5;
    }
    let mut sum: i64 = 0;
    for r in 0..n {
        for c in 0..n {
            let mut acc: i64 = 0;
            for k in 0..n {
                acc += a[r*n+k] * b[k*n+c];
            }
            sum += acc % 13;
        }
    }
    println!("{}", sum);
}
