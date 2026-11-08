// Diagnostic: are mut globals registered in dmir.globals?
use forgen::driver::ForgenCompiler;

#[test]
fn debug() {
    let src = std::fs::read_to_string("C:/Temp/mutglob.dtr").unwrap();
    let compiler = ForgenCompiler::new("release");
    let dmir = compiler.debug_lower_source(&src, "mutglob.dtr").unwrap();
    println!("GLOBALS: {:?}", dmir.globals);
    println!("ALL FNS: {:?}", dmir.functions.keys().collect::<Vec<_>>());
    assert!(!dmir.globals.is_empty(), "globals must be registered");
}
