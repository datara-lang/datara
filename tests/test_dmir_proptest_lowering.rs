//! Milestone 4 - DMIR lowering property tests (proptest).
//!
//! For randomly generated, depth-bounded integer arithmetic programs:
//! 1. Lowering to DMIR must succeed without panicking.
//! 2. The lowered module must satisfy the DMIR structural verifier
//!    (CFG integrity, SSA single assignment, use-before-def dominance).
//! 3. Lowering must be deterministic: two lowerings of the same source
//!    produce byte-identical debug representations.
//!
//! All compilations run inside a 64 MiB-stack thread (see bcfac07). Generated
//! expressions use only `+` / `*` with leaves <= 9 and nesting depth <= 4, so
//! the worst-case value 9^16 (~1.8e15) stays far inside i64 range.

use forgen::dmir::verify_module;
use forgen::driver::ForgenCompiler;
use proptest::prelude::*;

fn expression() -> BoxedStrategy<String> {
    let leaf = (1i64..=9).prop_map(|v| v.to_string());
    leaf.prop_recursive(4, 24, 10, |inner| {
        prop_oneof![
            (inner.clone(), inner.clone()).prop_map(|(a, b)| format!("({} + {})", a, b)),
            (inner.clone(), inner.clone()).prop_map(|(a, b)| format!("({} * {})", a, b)),
        ]
    })
    .boxed()
}

fn lower(source: String) -> Result<forgen::dmir::Module, String> {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let compiler = ForgenCompiler::new("release");
            compiler.compile_source_to_dmir(&source, "proptest_dmir.dtr")
        })
        .expect("failed to spawn compiler thread")
        .join()
        .expect("compiler thread panicked")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn lowering_is_structurally_sound_and_deterministic(expr in expression()) {
        let source = format!("fn main() {{\n    out {}\n}}\n", expr);

        let module = lower(source.clone())
            .unwrap_or_else(|e| panic!("lowering failed for `{}`: {}", expr, e));
        prop_assert!(
            module.functions.contains_key("main"),
            "lowered module must contain main"
        );
        prop_assert!(
            verify_module(&module).is_ok(),
            "DMIR verifier rejected lowering of `{}`: {:?}",
            expr,
            verify_module(&module)
        );

        let module2 = lower(source)
            .unwrap_or_else(|e| panic!("second lowering failed for `{}`: {}", expr, e));
        prop_assert_eq!(
            format!("{:?}", module),
            format!("{:?}", module2),
            "lowering must be deterministic"
        );
    }
}
