//! v1.4.4 Whole-Program Dead Code Elimination & Tree Shaking Tests.

use forgen::driver::ForgenCompiler;

#[test]
fn test_dead_function_elimination() {
    let src = r#"
fn alive_func() -> Int {
    return 42
}

fn dead_func() -> Int {
    return 999
}

fn main() -> Int {
    let x = alive_func()
    return x
}
"#;
    let compiler = ForgenCompiler::new("release");
    let module = compiler
        .compile_source_to_dmir(src, "test_dce_func.dtr")
        .expect("must compile to DMIR");

    println!("Functions in module: {:?}", module.functions.keys());
    assert!(module.functions.contains_key("main"), "main must be kept");
    assert!(
        !module.functions.contains_key("dead_func"),
        "dead_func must be eliminated by tree shaking"
    );
}

#[test]
fn test_dead_class_method_elimination_with_intrinsic_name() {
    let src = r#"
class DeadClass {
    id: Int
}

behavior DeadClass {
    len() -> Int {
        return 100
    }
    attack() -> Int {
        return 666
    }
}

fn main() -> Int {
    let list = [1, 2, 3]
    let n = list.len()
    return n
}
"#;
    let compiler = ForgenCompiler::new("release");
    let module = compiler
        .compile_source_to_dmir(src, "test_dce_class.dtr")
        .expect("must compile to DMIR");

    assert!(module.functions.contains_key("main"), "main must be kept");
    assert!(
        !module.functions.contains_key("DeadClass_len"),
        "DeadClass_len must be pruned even if list.len() was called"
    );
    assert!(
        !module.functions.contains_key("DeadClass_attack"),
        "DeadClass_attack must be pruned"
    );
}

#[test]
fn test_used_class_method_retained_unused_pruned() {
    let src = r#"
class Packet {
    size: Int
}

behavior Packet {
    @inline(never)
    parse() -> Int {
        return this.size
    }
    unused_dump() -> Int {
        return 9999
    }
}

fn main() -> Int {
    let p = Packet { size: 128 }
    let s = p.parse()
    return s
}
"#;
    let compiler = ForgenCompiler::new("release");
    let module = compiler
        .compile_source_to_dmir(src, "test_dce_method.dtr")
        .expect("must compile to DMIR");

    assert!(module.functions.contains_key("main"), "main must be kept");
    assert!(
        module.functions.contains_key("Packet_parse"),
        "Packet_parse must be kept because it is called"
    );
    assert!(
        !module.functions.contains_key("Packet_unused_dump"),
        "Packet_unused_dump must be eliminated by tree shaking"
    );
}

#[test]
fn test_aot_execution_with_dead_code_stripped() {
    let src = r#"
fn dead_worker() -> Int {
    return 1000
}

fn alive_add(a: Int, b: Int) -> Int {
    return a + b
}

fn main() -> Int {
    let res = alive_add(20, 22)
    print(int_to_str(res))
    return 0
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(src, "test_dce_run.dtr", None);
    assert!(
        res.success,
        "compilation failed: {:?}\n{}",
        res.error, res.diagnostics
    );

    let exe = res.exe_path.expect("must produce a native .exe");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");

    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    if code != 0 {
        eprintln!("STDERR: {}", stderr);
    }
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn test_unused_extern_functions_pruned() {
    use forgen::dmir::{BasicBlock, BasicBlockId, Function, Inst, Module, Terminator, ValueId};
    use forgen::optimizer::Optimizer;

    let mut module = Module::new("test_mod");
    let mut main_fn = Function::default();
    main_fn.name = "main".to_string();
    main_fn.blocks.push(BasicBlock {
        id: BasicBlockId(0),
        label: "entry".to_string(),
        params: vec![],
        instructions: vec![
            Inst::Call {
                dest: ValueId(1),
                func: "c_used_func".to_string(),
                args: vec![],
                ty: "Int".to_string(),
            },
        ],
        terminator: Terminator::Return { value: Some(ValueId(1)) },
    });
    module.functions.insert("main".to_string(), main_fn);
    module.extern_functions.insert("c_used_func".to_string(), (vec![], "Int".to_string()));
    module.extern_functions.insert("c_unused_func".to_string(), (vec![], "Int".to_string()));
    module.extern_sret.insert("c_unused_func".to_string(), 32);

    let mut optimizer = Optimizer::new("release");
    optimizer.optimize_module(&mut module).expect("must optimize");

    assert!(module.extern_functions.contains_key("c_used_func"), "used extern must be retained");
    assert!(!module.extern_functions.contains_key("c_unused_func"), "unused extern must be pruned");
    assert!(!module.extern_sret.contains_key("c_unused_func"), "unused sret must be pruned");
}

#[test]
fn test_minimal_binary_size() {
    let src = r#"
fn main() -> Int {
    print("hello")
    return 0
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(src, "test_min_size.dtr", None);
    assert!(res.success);
    let exe = res.exe_path.unwrap();
    let metadata = std::fs::metadata(&exe).unwrap();
    let size = metadata.len();
    println!("Compiled minimal release binary size: {} bytes ({:.2} KB)", size, size as f64 / 1024.0);
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
}

#[test]
fn test_tiny_mode_binary_size() {
    let src = r#"
fn main() -> Int {
    print("hello tiny")
    return 0
}
"#;
    let compiler = ForgenCompiler::new("tiny");
    let res = compiler.compile_source_native(src, "test_tiny_size.dtr", None);
    assert!(res.success);
    let exe = res.exe_path.unwrap();
    let metadata = std::fs::metadata(&exe).unwrap();
    let size = metadata.len();
    println!("Compiled tiny mode binary size: {} bytes ({:.2} KB)", size, size as f64 / 1024.0);
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
}



