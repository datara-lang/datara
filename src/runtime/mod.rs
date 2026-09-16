//! The C runtime that generated Datara programs link against.
//!
//! `build.rs` compiles `src/runtime/datara_runtime.c` into `OUT_DIR` and passes
//! its location to this crate through `DATARA_RUNTIME_LIB`. Nothing in the
//! compiler may reference the runtime by an absolute path: the checkout
//! location is not knowable at compile time, and a stale checked-in object file
//! silently diverges from its source (that is how a float-printing bug once
//! survived into released builds).

use std::path::PathBuf;

pub mod bump_arena;
pub mod fiber;
pub mod parallel;
pub mod scheduler;
pub mod zero_copy;

pub use scheduler::{
    DataraRtTaskNode, datara_rt_run_concurrent_timers, datara_rt_schedule_cancel,
    datara_rt_schedule_run, datara_rt_scheduler_mutex_queue_pushes,
    datara_rt_scheduler_reset_stats, datara_rt_scheduler_wave_executions, datara_rt_time_delta_ms,
    datara_rt_time_now_ms, datara_rt_time_now_ns, datara_rt_time_precise_ms,
    datara_rt_time_reset_delta, datara_rt_timer_cancel, datara_rt_timer_create,
    datara_rt_timer_wait,
};

/// Current ABI version of the Datara runtime expected by the compiler.
pub const COMPILER_DATARA_RT_ABI_VERSION: u32 = 1;

unsafe extern "C" {
    pub fn datara_rt_abi_version() -> u32;
    pub fn datara_rt_pool_alloc(sz: usize) -> *mut ();
    pub fn datara_rt_pool_free(ptr: *mut (), sz: usize);
    pub fn datara_rt_box_alloc(val: i64) -> *mut i64;
    pub fn datara_rt_box_get(b: *mut i64) -> i64;
    pub fn datara_rt_box_free(b: *mut i64);
    pub fn datara_rt_str_sso(s: *const std::ffi::c_char) -> *const std::ffi::c_char;
    pub fn datara_rt_str_is_sso(s: *const std::ffi::c_char) -> i64;
    pub fn datara_rt_heap_alloc_count() -> i64;
    pub fn datara_rt_reset_heap_alloc_count();
    pub fn datara_rt_list_init_stack(stack_buf: *mut (), cap: i64) -> *mut i64;
    pub fn datara_rt_list_is_small_vec(list: *mut i64) -> i64;
}

/// Verify that runtime ABI version matches the compiler's expected ABI version.
pub fn verify_runtime_abi_version(runtime_ver: u32, compiler_ver: u32) -> Result<(), String> {
    if runtime_ver != compiler_ver {
        return Err(format!(
            "ABI version mismatch: runtime v{} vs compiler v{}. Rebuild the runtime library or upgrade the compiler.",
            runtime_ver, compiler_ver
        ));
    }
    Ok(())
}

/// Check the linked runtime's ABI version against the compiler.
pub fn verify_runtime_abi() -> Result<(), String> {
    let rt_ver = unsafe { datara_rt_abi_version() };
    verify_runtime_abi_version(rt_ver, COMPILER_DATARA_RT_ABI_VERSION)
}

/// Path to the native runtime archive (`datara_runtime.lib` on MSVC,
/// `libdatara_runtime.a` elsewhere).
///
/// Uses `env!` rather than `option_env!` on purpose: if `build.rs` did not run,
/// the compiler would otherwise link nothing and every generated program would
/// fail much later with an unrelated missing-symbol error.
pub fn runtime_lib_path() -> PathBuf {
    let lib_name = runtime_lib_name();

    // 1. Fresh compile-time baked OUT_DIR path from build.rs (top priority)
    let baked = PathBuf::from(env!("DATARA_RUNTIME_LIB"));
    if baked.exists() {
        return baked;
    }

    // 2. Check relative to current executable (e.g. dist installation or portable zip)
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        let next_to_exe = exe_dir.join(lib_name);
        if next_to_exe.exists() {
            return next_to_exe;
        }
        let in_runtime_dir = exe_dir.join("runtime").join(lib_name);
        if in_runtime_dir.exists() {
            return in_runtime_dir;
        }
        let in_parent_runtime = exe_dir.parent().map(|p| p.join("runtime").join(lib_name));
        if let Some(p) = in_parent_runtime
            && p.exists()
        {
            return p;
        }
        if let Some(parent) = exe_dir.parent() {
            let in_parent_lib = parent.join("lib").join("datara").join(lib_name);
            if in_parent_lib.exists() {
                return in_parent_lib;
            }
            let in_parent_lib64 = parent.join("lib64").join("datara").join(lib_name);
            if in_parent_lib64.exists() {
                return in_parent_lib64;
            }
            let in_parent_share = parent
                .join("share")
                .join("datara")
                .join("runtime")
                .join(lib_name);
            if in_parent_share.exists() {
                return in_parent_share;
            }
        }
    }

    // 3. Check environment variables
    if let Ok(rt_env) = std::env::var("DATARA_RUNTIME") {
        let p = PathBuf::from(rt_env);
        if p.is_file() {
            return p;
        }
        let in_dir = p.join(lib_name);
        if in_dir.exists() {
            return in_dir;
        }
    }
    if let Ok(home) = std::env::var("DATARA_HOME") {
        let in_home_runtime = std::path::Path::new(&home).join("runtime").join(lib_name);
        if in_home_runtime.exists() {
            return in_home_runtime;
        }
        let in_home_root = std::path::Path::new(&home).join(lib_name);
        if in_home_root.exists() {
            return in_home_root;
        }
    }

    // 4. User profile, AppData, and ProgramFiles paths
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let in_user_datara = PathBuf::from(&home)
            .join(".datara")
            .join("runtime")
            .join(lib_name);
        if in_user_datara.exists() {
            return in_user_datara;
        }
        let in_user_root = PathBuf::from(home).join(".datara").join(lib_name);
        if in_user_root.exists() {
            return in_user_root;
        }
    }
    if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
        let in_local = PathBuf::from(local_app)
            .join("Programs")
            .join("Datara")
            .join("runtime")
            .join(lib_name);
        if in_local.exists() {
            return in_local;
        }
    }
    if let Ok(pf) = std::env::var("ProgramFiles") {
        let in_pf = PathBuf::from(pf)
            .join("Datara")
            .join("runtime")
            .join(lib_name);
        if in_pf.exists() {
            return in_pf;
        }
    }

    // 5. System standard paths (Linux, macOS, Unix packages)
    let sys_candidates = [
        PathBuf::from("/usr/lib/datara").join(lib_name),
        PathBuf::from("/usr/lib64/datara").join(lib_name),
        PathBuf::from("/usr/local/lib/datara").join(lib_name),
        PathBuf::from("/usr/share/datara/runtime").join(lib_name),
        PathBuf::from("/opt/homebrew/lib/datara").join(lib_name),
        PathBuf::from("/opt/datara/runtime").join(lib_name),
    ];
    for sc in sys_candidates {
        if sc.exists() {
            return sc;
        }
    }

    // 6. Fall back to baked path
    baked
}

/// Human-readable name of the runtime archive, for diagnostics.
pub fn runtime_lib_name() -> &'static str {
    if cfg!(target_env = "msvc") {
        "datara_runtime.lib"
    } else {
        "libdatara_runtime.a"
    }
}

/// Dynamically locates `datara_runtime.c` across development checkouts,
/// installed toolchains, and custom DATARA_HOME directories.
pub fn runtime_source_path() -> Option<PathBuf> {
    // 1. Compile-time manifest checkout
    let baked = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/runtime/datara_runtime.c"
    ));
    if baked.exists() {
        return Some(baked);
    }

    // 2. Relative to current executable
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        let candidates = [
            exe_dir.join("runtime").join("datara_runtime.c"),
            exe_dir.join("src").join("runtime").join("datara_runtime.c"),
            exe_dir.join("datara_runtime.c"),
        ];
        for c in candidates {
            if c.exists() {
                return Some(c);
            }
        }
        if let Some(parent) = exe_dir.parent() {
            let p_cands = [
                parent.join("runtime").join("datara_runtime.c"),
                parent.join("src").join("runtime").join("datara_runtime.c"),
                parent
                    .join("share")
                    .join("datara")
                    .join("runtime")
                    .join("datara_runtime.c"),
            ];
            for p in p_cands {
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }

    // 3. Check DATARA_HOME
    if let Ok(home) = std::env::var("DATARA_HOME") {
        let home_path = std::path::Path::new(&home);
        let candidates = [
            home_path.join("runtime").join("datara_runtime.c"),
            home_path
                .join("src")
                .join("runtime")
                .join("datara_runtime.c"),
            home_path.join("datara_runtime.c"),
        ];
        for c in candidates {
            if c.exists() {
                return Some(c);
            }
        }
    }

    // 4. User profile, AppData, and System paths
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let u_cands = [
            PathBuf::from(&home)
                .join(".datara")
                .join("runtime")
                .join("datara_runtime.c"),
            PathBuf::from(&home)
                .join(".datara")
                .join("src")
                .join("runtime")
                .join("datara_runtime.c"),
        ];
        for u in u_cands {
            if u.exists() {
                return Some(u);
            }
        }
    }
    if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
        let l_cand = PathBuf::from(local_app)
            .join("Programs")
            .join("Datara")
            .join("runtime")
            .join("datara_runtime.c");
        if l_cand.exists() {
            return Some(l_cand);
        }
    }
    let sys_candidates = [
        PathBuf::from("/usr/share/datara/runtime/datara_runtime.c"),
        PathBuf::from("/usr/local/share/datara/runtime/datara_runtime.c"),
        PathBuf::from("/opt/datara/runtime/datara_runtime.c"),
    ];
    for sc in sys_candidates {
        if sc.exists() {
            return Some(sc);
        }
    }

    // 5. Current working directory fallback
    let cwd_rel = PathBuf::from("src/runtime/datara_runtime.c");
    if cwd_rel.exists() {
        return Some(cwd_rel);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_lib_path_is_absolute_and_exists() {
        let path = runtime_lib_path();
        assert!(path.is_absolute(), "expected absolute path, got {:?}", path);
        assert!(
            path.exists(),
            "runtime archive missing at {:?}; run `cargo build` so build.rs regenerates it",
            path
        );
    }
}
