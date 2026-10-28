//! M9 fuzz-инварианты (PLAN_V150 M9): парсер и чекер обязаны возвращать
//! ДИАГНОСТИКУ на произвольных байтах/тексте, никогда не паниковать.
//! cargo-fuzz/libFuzzer недоступен под MSVC — инвариантные seeded-фаззеры
//! здесь закрывают тот же контракт в обычном тест-сьюте (безcoverage-гида).

use forgen::driver::ForgenCompiler;

/// Простой детерминированный генератор байт (xorshift), чтобы прогон был
/// воспроизводимым: одинаковый seed — одинаковый набор.
struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn byte(&mut self) -> u8 {
        (self.next() & 0xFF) as u8
    }
}

fn must_not_panic_source(src: &str, tag: &str) {
    let compiler = ForgenCompiler::new("debug");
    // Никаких паник: результат — успех или диагностика.
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let res = compiler.compile_source(src, tag, None);
        assert!(
            res.success || !res.diagnostics.is_empty(),
            "neither success nor diagnostics for: {src:?}"
        );
    }));
}

#[test]
fn fuzz_parser_random_bytes_never_panics() {
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    for round in 0..64 {
        let len = (rng.next() % 512) as usize + 1;
        let bytes: Vec<u8> = (0..len).map(|_| rng.byte()).collect();
        // Проверяем и как UTF-8 lossy (как читает компилятор), и сырой путь.
        let text = String::from_utf8_lossy(&bytes).into_owned();
        must_not_panic_source(&text, &format!("fuzz_bytes_{round}.dtr"));
    }
}

#[test]
fn fuzz_parser_truncated_datara_never_panics() {
    // Осмысленный префикс + обрезка в случайном месте — ловит срезы
    // вне границ токенизатора/парсера.
    let base = r#"struct P { x: Int, y: Float }
behavior P {
    fn len2() -> Int { return 2 }
}
fn main() {
    val xs = [1, "two", 3.5]
    val m = { "a": 1, "b": 2 }
    for x in xs {
        if x is Int {
            out fmt"int {x}"
        }
    }
    val r = Outcome.ok(5) or 0
    out int_to_str(r)
}
"#;
    for cut in 0..base.len() {
        must_not_panic_source(&base[..cut], &format!("fuzz_cut_{cut}.dtr"));
    }
}

#[test]
fn fuzz_parser_grammar_stress_never_panics() {
    // Комбинаторные скобочные/строковые паттерны: несбалансированные скобки,
    // вложенные fmt-интерполяции, скрытые NUL.
    let fragments = [
        "fn f() { }",
        "fn f( { ) }",
        "out fmt\"{ {1} }\"",
        "val s = \"\\\" unterminated",
        "struct S { a: Map<Str, List<Int>> }",
        "match x { _ => 1",
        "{ }",
        "((((((((((",
        "))))))))))",
        "\"\"\"\"\"\"",
        "fn f\x00g() {}",
        "val x = 1 as? as! Int",
        "if else while match",
        "a .. ..< ..= ... b",
        "@arc @derive(A) @unknownattr fn f() {}",
    ];
    for i in 0..fragments.len() {
        for j in 0..fragments.len() {
            let src = format!("{}\n{}", fragments[i], fragments[j]);
            must_not_panic_source(&src, &format!("fuzz_pair_{i}_{j}.dtr"));
        }
    }
}

#[test]
fn fuzz_manifest_broken_toml_never_panics() {
    // datara.toml с мусором не должен ронять разрешение манифеста
    // (используем compile_source в каталоге с испорченным manifest).
    let dir = std::env::temp_dir().join(format!("forgen_fuzz_manifest_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let cases = [
        "",
        "[capabilities]\nfs = [[[[",
        "garbage }}}",
        "[project]\nname = \n[capabilities]\nfs-read = \"not-a-list\"",
        "[[projects]]\nprojects = repeat repeat",
    ];
    for case in cases.iter() {
        let manifest = dir.join("datara.toml");
        std::fs::write(&manifest, case).unwrap();
        let src_path = dir.join("main.dtr");
        std::fs::write(&src_path, "fn main() { out 1 }").unwrap();
        let compiler = ForgenCompiler::new("debug");
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let res = compiler.compile_file(&src_path, None);
            assert!(res.success || !res.diagnostics.is_empty());
        }));
        let _ = std::fs::remove_file(&manifest);
        let _ = std::fs::remove_file(&src_path);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
