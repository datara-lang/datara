fn main() {
    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < 100000000 {
        acc += i & 255;
        i += 1;
    }
    println!("{}", acc);
}
