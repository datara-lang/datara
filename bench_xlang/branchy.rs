fn heavy(acc: i64, i: i64) -> i64 {
    let mut h = acc;
    let mut k = 0;
    while k < 16 { h += (i ^ k) & 63; k += 1; }
    h
}
fn main() {
    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < 50000000 {
        if (i & 1023) == 0 { acc = heavy(acc, i); } else { acc += 1; }
        i += 1;
    }
    println!("{}", acc);
}
