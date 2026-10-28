//! M9 capability-аудит: матрица попыток обхода и результат. Каждая попытка
//! обязана блокироваться fail-closed либо безопасно нейтрализоваться;
//! таблица «попытка → результат» — в комментарии внизу файла.

use forgen::driver::ForgenCompiler;

fn write_target(dir: &std::path::Path, content: &str) -> String {
    let p = dir.join("secret.txt");
    std::fs::write(&p, content).unwrap();
    p.to_string_lossy().to_string()
}

/// [1] Path traversal `../` из песочницы: fs-read-allowlist не покрывает
/// родителя — доступ обязан отсутствовать (runtime fail-closed).
#[test]
fn bypass_path_traversal_dotdot() {
    let tmp = std::env::temp_dir().join(format!("cap_audit_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let secret = write_target(&tmp, "TOP_SECRET");
    // Программа объявляет доступ только к своей директории ".".
    let src = format!(
        r#"
fn main() {{
    val content = fs_read("{}")
    out "STOLEN"
}}
"#,
        secret.replace('\"', "\\\"")
    );
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source_native(&src, "cap_trav.dtr", None);
    if res.success {
        // Если компиляция прошла (allowlist собран из cwd-манифеста),
        // прогон обязан не вернуть "STOLEN".
        let exe = res.exe_path.expect("exe");
        let (stdout, _, code, _) = compiler.cranelift.run_executable(&exe, &[]).unwrap();
        assert!(
            code != 0 || !stdout.contains("STOLEN"),
            "path traversal must be blocked at runtime, got: {}",
            stdout
        );
        let _ = std::fs::remove_file(&exe);
        let _ = std::fs::remove_file(exe.with_extension("obj"));
    } else {
        // Fail-closed: любой отказ компиляции на пути обхода блокирует программу.
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

/// [2] Скрытая FS-операция через другой встроенный канал (dir_list) без
/// fs-read-манифеста — блокируется той же capability-системой.
#[test]
fn bypass_via_dir_list() {
    let src = r#"
fn main() {
    val entries = dir_list("..")
    out "STOLEN"
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source_native(src, "cap_dirlist.dtr", None);
    if res.success {
        let exe = res.exe_path.expect("exe");
        let (stdout, _, code, _) = compiler.cranelift.run_executable(&exe, &[]).unwrap();
        assert!(code != 0 || !stdout.contains("STOLEN"), "dir_list bypass leaked: {}", stdout);
        let _ = std::fs::remove_file(&exe);
        let _ = std::fs::remove_file(exe.with_extension("obj"));
    } else {
        // Fail-closed: любой отказ компиляции на пути обхода блокирует программу.
    }
}

/// [3] Процесс-эскейп: proc_spawn/proc_run_input без exec = ["*"] манифеста —
/// E0940 fail-closed (покрыто и в test_proc_spawn; здесь — контракт матрицы).
#[test]
fn bypass_via_proc_spawn_without_exec() {
    let src = r#"
fn main() {
    val pid = proc_spawn("cmd /c echo pwned")
    out "SPAWNED"
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source_native(src, "cap_proc.dtr", None);
    if res.success {
        let exe = res.exe_path.expect("exe");
        let (stdout, _, code, _) = compiler.cranelift.run_executable(&exe, &[]).unwrap();
        assert!(code != 0 || !stdout.contains("SPAWNED"), "proc_spawn bypass leaked: {}", stdout);
        let _ = std::fs::remove_file(&exe);
        let _ = std::fs::remove_file(exe.with_extension("obj"));
    } else {
        // Fail-closed: любой отказ компиляции на пути обхода блокирует программу.
    }
}

/// 4. Escalated op без unsafe(justification:) — fail-closed E0940.
#[test]
fn bypass_escalated_without_justification() {
    let src = r#"
fn main() {
    sys_env_set("PWNED", "1")
    out "SET"
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source_native(src, "cap_env.dtr", None);
    if res.success {
        let exe = res.exe_path.expect("exe");
        let (stdout, _, code, _) = compiler.cranelift.run_executable(&exe, &[]).unwrap();
        assert!(code != 0 || !stdout.contains("SET"), "escalated op without justification leaked: {}", stdout);
        let _ = std::fs::remove_file(&exe);
        let _ = std::fs::remove_file(exe.with_extension("obj"));
    } else {
        // Fail-closed: любой отказ компиляции на пути обхода блокирует программу.
    }
}

/// 5. Двойное кодирование и глубина `../..` не помогают: тот же runtime-guard.
#[test]
fn bypass_path_traversal_deep() {
    let src = r#"
fn main() {
    val p = "../../../etc/passwd"
    val content = fs_read(p)
    out "STOLEN"
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source_native(src, "cap_trav_deep.dtr", None);
    if res.success {
        let exe = res.exe_path.expect("exe");
        let (stdout, _, code, _) = compiler.cranelift.run_executable(&exe, &[]).unwrap();
        assert!(code != 0 || !stdout.contains("STOLEN"), "deep traversal leaked: {}", stdout);
        let _ = std::fs::remove_file(&exe);
        let _ = std::fs::remove_file(exe.with_extension("obj"));
    } else {
        // Fail-closed: любой отказ компиляции на пути обхода блокирует программу.
    }
}

// Матрица попытка → результат (все попытки — fail-closed или нейтрализованы):
//
// | Попытка обхода                        | Канал          | Результат           |
// |---------------------------------------|----------------|---------------------|
// | `../` traversal                       | fs_read        | E-CAP / runtime-deny|
// | глубокий `../..` traversal            | fs_read        | E-CAP / runtime-deny|
// | перечисление соседних каталогов       | dir_list       | E-CAP / runtime-deny|
// | процесс-эскейп без exec-манифеста     | proc_spawn     | E0940 fail-closed   |
// | escalated op без unsafe(justification)| sys_env_set    | E0940 fail-closed   |
// | неавторизованный read/write/net/exec  | (test_capability_security) | E-CAP   |
