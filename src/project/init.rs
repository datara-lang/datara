use super::manifest::DataraManifest;
use std::fs;
use std::path::Path;

pub struct ProjectInitializer;

impl ProjectInitializer {
    /// Initializes a standard Level 3 Datara project
    pub fn init(name: Option<&str>, target_dir: &Path) -> Result<(), String> {
        let dir_name = if let Some(n) = name {
            n.to_string()
        } else {
            target_dir
                .canonicalize()
                .ok()
                .and_then(|p| {
                    p.file_name()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| "datara_app".to_string())
        };

        let project_root = if name.is_some() {
            target_dir.join(&dir_name)
        } else {
            target_dir.to_path_buf()
        };

        if !project_root.exists() {
            fs::create_dir_all(&project_root).map_err(|e| {
                format!(
                    "Failed to create project directory '{}': {}",
                    project_root.display(),
                    e
                )
            })?;
        }

        // 1. datara.toml manifest
        let manifest_path = project_root.join("datara.toml");
        if !manifest_path.exists() {
            let content = DataraManifest::default_template(&dir_name);
            fs::write(&manifest_path, content)
                .map_err(|e| format!("Failed to write datara.toml: {}", e))?;
        }

        // 2. src/main.dtr
        let src_dir = project_root.join("src");
        fs::create_dir_all(&src_dir).map_err(|e| format!("Failed to create src dir: {}", e))?;
        let main_path = src_dir.join("main.dtr");
        if !main_path.exists() {
            let main_content = format!(
                "// Main entry point for {}\nfn main() {{\n    out \"Hello from Datara!\"\n}}\n",
                dir_name
            );
            fs::write(&main_path, main_content)
                .map_err(|e| format!("Failed to write src/main.dtr: {}", e))?;
        }

        // 3. tests/test_main.dtr
        let tests_dir = project_root.join("tests");
        fs::create_dir_all(&tests_dir).map_err(|e| format!("Failed to create tests dir: {}", e))?;
        let test_path = tests_dir.join("test_main.dtr");
        if !test_path.exists() {
            let test_content = "// Integration test for application\nfn test_addition() -> Int {\n    return 10 + 20\n}\n\nfn main() {\n    let res = test_addition()\n    if res == 30 {\n        out \"PASS: test_addition\"\n    } else {\n        err \"FAIL: test_addition\"\n    }\n}\n";
            fs::write(&test_path, test_content)
                .map_err(|e| format!("Failed to write tests/test_main.dtr: {}", e))?;
        }

        // 4. examples/demo.dtr
        let examples_dir = project_root.join("examples");
        fs::create_dir_all(&examples_dir)
            .map_err(|e| format!("Failed to create examples dir: {}", e))?;
        let example_path = examples_dir.join("demo.dtr");
        if !example_path.exists() {
            let example_content = "// Example usage demo\nstruct Greeter {\n    name: Str\n}\n\nbehavior Greeter {\n    fn greet() {\n        out \"Welcome to \" + self.name\n    }\n}\n\nfn main() {\n    let app = Greeter { name: \"Datara\" }\n    app.greet()\n}\n";
            fs::write(&example_path, example_content)
                .map_err(|e| format!("Failed to write examples/demo.dtr: {}", e))?;
        }

        // 5. .gitignore
        let gitignore_path = project_root.join(".gitignore");
        if !gitignore_path.exists() {
            let gitignore_content = "target/\n*.exe\n*.obj\n*.pdb\n*.pgo.json\n";
            let _ = fs::write(&gitignore_path, gitignore_content);
        }

        println!(
            "Created Datara package '{}' at '{}'",
            dir_name,
            project_root.display()
        );
        Ok(())
    }

    /// Initializes a Datara Community Library package
    pub fn init_lib(name: Option<&str>, target_dir: &Path) -> Result<(), String> {
        let dir_name = if let Some(n) = name {
            n.to_string()
        } else {
            target_dir
                .canonicalize()
                .ok()
                .and_then(|p| {
                    p.file_name()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| "datara_lib".to_string())
        };

        let project_root = if name.is_some() {
            target_dir.join(&dir_name)
        } else {
            target_dir.to_path_buf()
        };

        if !project_root.exists() {
            fs::create_dir_all(&project_root).map_err(|e| {
                format!(
                    "Failed to create library directory '{}': {}",
                    project_root.display(),
                    e
                )
            })?;
        }

        // 1. datara.toml library manifest
        let manifest_path = project_root.join("datara.toml");
        if !manifest_path.exists() {
            let content = format!(
                "[package]\nname = \"{}\"\nversion = \"1.0.0\"\ntype = \"lib\"\nauthors = []\ndescription = \"A high-performance Datara community library\"\n\n[exports]\nroot = \"src/lib.dtr\"\n",
                dir_name
            );
            fs::write(&manifest_path, content)
                .map_err(|e| format!("Failed to write datara.toml: {}", e))?;
        }

        // 2. src/lib.dtr
        let src_dir = project_root.join("src");
        fs::create_dir_all(&src_dir).map_err(|e| format!("Failed to create src dir: {}", e))?;
        let lib_path = src_dir.join("lib.dtr");
        if !lib_path.exists() {
            let lib_content = format!(
                "// Datara Library: {}\nstruct Helper {{\n    scale_factor: Int\n}}\n\nbehavior Helper {{\n    multiply(val: Int) -> Int => val * this.scale_factor\n}}\n\nfn create_helper(scale: Int) -> Helper {{\n    return Helper {{ scale_factor: scale }}\n}}\n",
                dir_name
            );
            fs::write(&lib_path, lib_content)
                .map_err(|e| format!("Failed to write src/lib.dtr: {}", e))?;
        }

        // 3. tests/test_lib.dtr
        let tests_dir = project_root.join("tests");
        fs::create_dir_all(&tests_dir).map_err(|e| format!("Failed to create tests dir: {}", e))?;
        let test_path = tests_dir.join("test_lib.dtr");
        if !test_path.exists() {
            let test_content = format!(
                "// Integration tests for library {}\nuse src.lib\n\nfn main() {{\n    let helper = create_helper(5)\n    let res = helper.multiply(10)\n    if res == 50 {{\n        out \"PASS: library tests\"\n    }}\n}}\n",
                dir_name
            );
            fs::write(&test_path, test_content)
                .map_err(|e| format!("Failed to write tests/test_lib.dtr: {}", e))?;
        }

        // 4. README.md
        let readme_path = project_root.join("README.md");
        if !readme_path.exists() {
            let readme_content = format!(
                "# {}\n\nA high-performance Datara community library.\n\n## Usage\n\n```datara\nuse {}\n\nfn main() {{\n    let helper = create_helper(2)\n    out helper.multiply(21)\n}}\n```\n",
                dir_name, dir_name
            );
            let _ = fs::write(&readme_path, readme_content);
        }

        // 5. .gitignore
        let gitignore_path = project_root.join(".gitignore");
        if !gitignore_path.exists() {
            let gitignore_content = "target/\n*.exe\n*.obj\n*.dll\n*.lib\n";
            let _ = fs::write(&gitignore_path, gitignore_content);
        }

        println!(
            "[Forgen] Successfully created Datara library '{}' at '{}'",
            dir_name,
            project_root.display()
        );
        Ok(())
    }

    /// Initializes a Bare-Metal Datara project (Cortex-M)
    pub fn init_bare(
        name: Option<&str>,
        target_dir: &Path,
        mcu: Option<&str>,
    ) -> Result<(), String> {
        let dir_name = if let Some(n) = name {
            n.to_string()
        } else {
            target_dir
                .canonicalize()
                .ok()
                .and_then(|p| {
                    p.file_name()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| "cortex_m_app".to_string())
        };

        let project_root = if name.is_some() {
            target_dir.join(&dir_name)
        } else {
            target_dir.to_path_buf()
        };

        if !project_root.exists() {
            fs::create_dir_all(&project_root).map_err(|e| {
                format!(
                    "Failed to create bare-metal project directory '{}': {}",
                    project_root.display(),
                    e
                )
            })?;
        }

        let mcu_name = mcu.unwrap_or("cortex-m4");

        // 1. datara.toml manifest
        let manifest_path = project_root.join("datara.toml");
        if !manifest_path.exists() {
            let content = format!(
                r#"[package]
name = "{}"
version = "0.1.0"
profile = "bare"

[target]
arch = "thumbv7em-none-eabihf"
cpu = "{}"

[devices]
allowed = ["GPIOC", "RCC"]
"#,
                dir_name, mcu_name
            );
            fs::write(&manifest_path, content)
                .map_err(|e| format!("Failed to write datara.toml: {}", e))?;
        }

        // 2. src/main.dtr
        let src_dir = project_root.join("src");
        fs::create_dir_all(&src_dir).map_err(|e| format!("Failed to create src dir: {}", e))?;
        let main_path = src_dir.join("main.dtr");
        if !main_path.exists() {
            let main_content = r#"@mmio(0x40021000)
struct RCC {
    cr: Int at 0x00,
    cfgr: Int at 0x08,
}

@mmio(0x40020800)
struct GPIOC {
    moder: Int at 0x00,
    odr: Int at 0x14,
}

fn main() {
    unsafe(justification: "Initialize hardware clocks and GPIO") {
        RCC.cr = 0x01
        GPIOC.moder = 0x1000
    }
}
"#;
            fs::write(&main_path, main_content)
                .map_err(|e| format!("Failed to write src/main.dtr: {}", e))?;
        }

        // 3. src/vectors.dtr
        let vectors_path = src_dir.join("vectors.dtr");
        if !vectors_path.exists() {
            let vectors_content = r#"fn Reset_Handler() {
    main()
}

fn Default_Handler() {
    while true {}
}
"#;
            fs::write(&vectors_path, vectors_content)
                .map_err(|e| format!("Failed to write src/vectors.dtr: {}", e))?;
        }

        // 4. memory.ld
        let ld_path = project_root.join("memory.ld");
        if !ld_path.exists() {
            let ld_content = r#"MEMORY
{
    FLASH (rx) : ORIGIN = 0x08000000, LENGTH = 512K
    RAM (rwx)  : ORIGIN = 0x20000000, LENGTH = 128K
}

SECTIONS
{
    .text : {
        *(.vectors)
        *(.text*)
        *(.rodata*)
    } > FLASH

    .data : {
        *(.data*)
    } > RAM AT > FLASH

    .bss : {
        *(.bss*)
    } > RAM
}
"#;
            fs::write(&ld_path, ld_content)
                .map_err(|e| format!("Failed to write memory.ld: {}", e))?;
        }

        // 5. .gitignore
        let gitignore_path = project_root.join(".gitignore");
        if !gitignore_path.exists() {
            let gitignore_content = "target/\n*.bin\n*.elf\n*.hex\n*.obj\n*.o\n";
            let _ = fs::write(&gitignore_path, gitignore_content);
        }

        println!(
            "[Forgen] Successfully created bare-metal project '{}' for {} at '{}'",
            dir_name,
            mcu_name,
            project_root.display()
        );
        Ok(())
    }
}

// ---- PLAN_V153 D1: project templates (--template cli|server|script) ----

pub const TEMPLATE_CLI: &str = "cli";
pub const TEMPLATE_SERVER: &str = "server";
pub const TEMPLATE_SCRIPT: &str = "script";

impl ProjectInitializer {
    /// Initializes a project from a PLAN_V153 D1 template. The layout is
    /// the standard Level-3 app (datara.toml + src/main.dtr + tests), with
    /// main.dtr pre-filled for the template's purpose.
    pub fn init_template(
        name: Option<&str>,
        target_dir: &Path,
        template: &str,
    ) -> Result<(), String> {
        // Reuse the standard layout by calling init, then overwrite main.dtr
        // with the template body (init does not clobber an existing main.dtr,
        // so write it AFTER init and unconditionally for a FRESH project).
        Self::init(name, target_dir)?;
        let dir_name = name.unwrap_or("datara_app");
        let main_path = target_dir.join(dir_name).join("src").join("main.dtr");
        let body = match template {
            TEMPLATE_CLI => format!(
                "// {name} — CLI tool (stdlib.cli flag parsing)
use stdlib.cli.cli

fn main() {{
    val argv = args()
    val a = parse_args(argv.slice(1, argv.len()))
    if a.has(\"help\") {{
        out \"Usage: {name} --name <who>\"
        return
    }}
    out \"Hello, \" + a.get(\"--name\")
}}
",
                name = dir_name
            ),
            TEMPLATE_SERVER => format!(
                "// {name} — TCP echo server (stdlib.net + fibers)
use stdlib.net.net

fn serve(l: Int) {{
    val acc = tcp_accept(l)
    if acc.is_success {{
        val peer = acc.value
        val req = tcp_recv(peer, 1024, 5000)
        if req.is_success {{
            tcp_send(peer, \"echo:\" + req.value)
        }}
        tcp_close(peer)
    }}
    tcp_close(l)
}}

fn main() {{
    unsafe(justification: \"echo server: local TCP\") {{
        val bound = tcp_listen(8080)
        if bound.is_success {{
            out \"Listening on 127.0.0.1:8080\"
            fiber_spawn(serve, bound.value)
            fiber_run_until_done()
        }}
    }}
}}
",
                name = dir_name
            ),
            TEMPLATE_SCRIPT => format!(
                "// {name} — script template (fs + time)
use stdlib.time.clock

fn main() {{
    out fmt_date(now_unix_ms())
    // args/env/file IO work out of the box:
    //   val argv = args()
    //   val text = read_file(\"input.txt\")
}}
",
                name = dir_name
            ),
            other => {
                return Err(format!(
                    "Unknown template '{}' (known: cli, server, script)",
                    other
                ));
            }
        };
        fs::write(&main_path, body)
            .map_err(|e| format!("Failed to write src/main.dtr: {}", e))?;
        println!("Template '{}' applied", template);
        Ok(())
    }
}
