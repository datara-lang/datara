//! v1.4.5: Bridge library loader for the JIT.
//!
//! Rust-bridge cdylibs built via `forgen pkg rust-bridge` export `extern "C"`
//! trampolines that Datara programs call through `extern fn` declarations.
//! The JIT resolves runtime symbols from its internal table first; any symbol
//! that remains unresolved (e.g. `regex_is_match`) falls back to this loader,
//! which scans well-known bridge directories and resolves the name against
//! every loaded cdylib. Loading happens once per JIT session; handles stay
//! alive for the process lifetime, which is exactly what Cranelift's
//! `symbol_lookup_fn` requires.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A set of loaded bridge dynamic libraries plus near-JIT trampolines.
///
/// Bridge cdylibs load at arbitrary OS-chosen addresses, frequently farther
/// than x86-64's ±2 GiB direct-call range from the JIT code (which the
/// near-memory provider deliberately keeps close to the host binary). Raw
/// DLL addresses therefore cannot feed Cranelift's PC-relative relocations
/// directly. Instead every resolved symbol gets a 12-byte thunk
/// (`mov rax, imm64; jmp rax`) allocated in executable memory near the host
/// binary; the thunk is reachable by near-call while its target can be
/// anywhere in the address space.
pub struct BridgeLibs {
    /// Raw OS module handles (HMODULE on Windows, dlopen handle on Unix),
    /// kept as usize so the struct stays `Send`.
    handles: Vec<usize>,
    /// symbol name -> thunk address. Empty when thunks were not installed.
    thunks: HashMap<String, usize>,
    /// (block ptr, len) of the executable thunk arena, for bookkeeping only.
    thunk_block: Option<(usize, usize)>,
}

impl BridgeLibs {
    /// Empty library set — lookup always fails.
    pub fn empty() -> Self {
        Self {
            handles: Vec::new(),
            thunks: HashMap::new(),
            thunk_block: None,
        }
    }

    /// True when at least one bridge library was loaded.
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }

    /// Resolve a symbol across all loaded bridge libraries.
    /// Returns the near-JIT thunk address when thunks are installed.
    pub fn lookup(&self, name: &str) -> Option<*const u8> {
        if let Some(&thunk) = self.thunks.get(name) {
            return Some(thunk as *const u8);
        }
        let c_name = match std::ffi::CString::new(name) {
            Ok(c) => c,
            Err(_) => return None,
        };
        for &h in &self.handles {
            if let Some(p) = lookup_symbol(h, &c_name) {
                return Some(p);
            }
        }
        None
    }

    /// Install near-JIT trampolines for `names`. `near_ref` is any address
    /// inside the host binary (the same anchor the near-memory allocator
    /// uses) so the thunks land within the ±2 GiB direct-call window of the
    /// JIT code. Returns the number of thunks installed. Symbols that fail
    /// to resolve stay absent — the JIT will panic on them with the usual
    /// "can't resolve symbol" message.
    pub fn install_thunks(&mut self, names: &[String], near_ref: usize) -> usize {
        if self.thunks.is_empty() {
            // Only build the arena once.
            let resolved: Vec<(String, usize)> = names
                .iter()
                .filter_map(|n| {
                    let c = std::ffi::CString::new(n.as_str()).ok()?;
                    for &h in &self.handles {
                        if let Some(p) = lookup_symbol(h, &c) {
                            return Some((n.clone(), p as usize));
                        }
                    }
                    None
                })
                .collect();
            if resolved.is_empty() {
                return 0;
            }
            const THUNK_SIZE: usize = 12;
            let total = resolved.len() * THUNK_SIZE;
            let block = match allocate_exec_near(total, near_ref) {
                Some(b) => b,
                None => return 0,
            };
            self.thunk_block = Some(block);
            let base = block.0 as *mut u8;
            for (i, (_name, target)) in resolved.iter().enumerate() {
                let at = unsafe { base.add(i * THUNK_SIZE) };
                // mov rax, imm64 ; jmp rax  (x86-64, works on any address)
                let code: [u8; 12] = [
                    0x48,
                    0xB8,
                    (*target & 0xFF) as u8,
                    ((*target >> 8) & 0xFF) as u8,
                    ((*target >> 16) & 0xFF) as u8,
                    ((*target >> 24) & 0xFF) as u8,
                    ((*target >> 32) & 0xFF) as u8,
                    ((*target >> 40) & 0xFF) as u8,
                    ((*target >> 48) & 0xFF) as u8,
                    ((*target >> 56) & 0xFF) as u8,
                    0xFF,
                    0xE0,
                ];
                unsafe {
                    std::ptr::copy_nonoverlapping(code.as_ptr(), at, THUNK_SIZE);
                }
            }
            if let Err(_e) = make_executable(base, total) {
                // Leave the block non-executable -> lookups would crash;
                // fail safe by keeping the thunk table empty.
                self.thunk_block = None;
                return 0;
            }
            for (i, (name, _target)) in resolved.iter().enumerate() {
                let addr = base as usize + i * THUNK_SIZE;
                self.thunks.insert(name.clone(), addr);
            }
        }
        self.thunks.len()
    }
}

impl Drop for BridgeLibs {
    fn drop(&mut self) {
        // Intentionally leak the handles: JIT code may still reference
        // symbols inside these modules for the lifetime of the process,
        // and unloading them would dangle every resolved pointer.
        self.handles.clear();
    }
}

/// Well-known bridge search locations, in priority order:
/// - `$FORGEN_BRIDGE_PATH` entries (path-separator separated)
/// - `<base>/bridges/*/` and `<cwd>/bridges/*/` (the `forgen pkg
///   rust-bridge` default out-dir)
/// - `<base>/dpm_packages/*/` and `<cwd>/dpm_packages/*/` (installed DPM
///   bridge packages)
/// - `<base>` and `<cwd>` themselves (a cdylib dropped next to the source)
pub fn collect_bridge_search_dirs(base_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    if let Ok(env_path) = std::env::var("FORGEN_BRIDGE_PATH") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        for part in env_path.split(sep) {
            let p = PathBuf::from(part);
            if p.is_dir() && !dirs.contains(&p) {
                dirs.push(p);
            }
        }
    }

    let roots: Vec<PathBuf> = [
        base_dir.map(|b| b.to_path_buf()),
        std::env::current_dir().ok(),
    ]
    .into_iter()
    .flatten()
    .collect();

    for root in &roots {
        // A cdylib placed directly next to the source/project.
        if root.is_dir() && !dirs.contains(root) {
            dirs.push(root.clone());
        }
        // Default rust-bridge out-dir layout: bridges/<crate>_bridge/
        let bridges = root.join("bridges");
        if let Ok(entries) = std::fs::read_dir(&bridges) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() && !dirs.contains(&p) {
                    dirs.push(p);
                }
            }
        }
        // DPM bridge packages: dpm_packages/<name>/
        let dpm = root.join("dpm_packages");
        if let Ok(entries) = std::fs::read_dir(&dpm) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() && !dirs.contains(&p) {
                    dirs.push(p);
                }
            }
        }
    }

    dirs
}

/// Load every dynamic library found in the given directories.
/// Library files are `*.dll` (Windows), `*.so` (Unix), `*.dylib` (macOS).
/// Files that fail to load (non-cdylib binaries, incompatible images) are
/// skipped silently — a bridge dir may contain build artifacts too.
pub fn load_bridge_libraries(dirs: &[PathBuf]) -> BridgeLibs {
    let mut handles: Vec<usize> = Vec::new();
    for dir in dirs {
        let candidates = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for entry in candidates.flatten() {
            let path = entry.path();
            if !path.is_file() || !is_dylib_path(&path) {
                continue;
            }
            if let Some(handle) = load_library(&path) {
                handles.push(handle);
            }
        }
    }
    BridgeLibs {
        handles,
        thunks: HashMap::new(),
        thunk_block: None,
    }
}

/// Allocate `size` bytes of executable memory near `near_ref` (same anchor
/// discipline as `near_memory::allocate_near_block`: probe ±2 GiB downward
/// first, then upward, falling back to an arbitrary allocation).
fn allocate_exec_near(size: usize, near_ref: usize) -> Option<(usize, usize)> {
    let gran = 64 * 1024usize;
    let alloc_size = size.div_ceil(gran) * gran;
    #[cfg(windows)]
    {
        const MEM_COMMIT: u32 = 0x1000;
        const MEM_RESERVE: u32 = 0x2000;
        const PAGE_READWRITE: u32 = 0x04;
        let max_steps = (800 * 1024 * 1024) / gran;
        for step in 1..max_steps {
            if near_ref > step * gran + 0x10000 {
                let target = (near_ref - step * gran) & !(gran - 1);
                let ptr = unsafe {
                    VirtualAlloc(
                        target as *const _,
                        alloc_size,
                        MEM_COMMIT | MEM_RESERVE,
                        PAGE_READWRITE,
                    )
                };
                if !ptr.is_null() {
                    return Some((ptr as usize, alloc_size));
                }
            }
            let target = (near_ref + step * gran) & !(gran - 1);
            let ptr = unsafe {
                VirtualAlloc(
                    target as *const _,
                    alloc_size,
                    MEM_COMMIT | MEM_RESERVE,
                    PAGE_READWRITE,
                )
            };
            if !ptr.is_null() {
                return Some((ptr as usize, alloc_size));
            }
        }
        let ptr = unsafe {
            VirtualAlloc(
                std::ptr::null(),
                alloc_size,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        };
        if ptr.is_null() {
            None
        } else {
            Some((ptr as usize, alloc_size))
        }
    }
    #[cfg(unix)]
    {
        let max_steps = (800 * 1024 * 1024) / gran;
        for step in 1..max_steps {
            if near_ref > step * gran + 0x10000 {
                let target = (near_ref - step * gran) & !(gran - 1);
                if let Some(p) = bridge_mmap_near(target, alloc_size) {
                    return Some(p);
                }
            }
            let target = (near_ref + step * gran) & !(gran - 1);
            if let Some(p) = bridge_mmap_near(target, alloc_size) {
                return Some(p);
            }
        }
        bridge_mmap_near(0, alloc_size)
    }
}

#[cfg(windows)]
unsafe extern "system" {
    fn VirtualAlloc(
        lpAddress: *const std::ffi::c_void,
        dwSize: usize,
        flAllocationType: u32,
        flProtect: u32,
    ) -> *mut std::ffi::c_void;
    fn VirtualProtect(
        lpAddress: *mut std::ffi::c_void,
        dwSize: usize,
        flNewProtect: u32,
        lpflOldProtect: *mut u32,
    ) -> i32;
}

#[cfg(windows)]
fn make_executable(ptr: *mut u8, len: usize) -> Result<(), String> {
    const PAGE_EXECUTE_READ: u32 = 0x20;
    let mut old_prot = 0u32;
    if unsafe { VirtualProtect(ptr as *mut _, len, PAGE_EXECUTE_READ, &mut old_prot) } == 0 {
        return Err("VirtualProtect failed".to_string());
    }
    Ok(())
}

#[cfg(unix)]
fn bridge_mmap_near(target: usize, len: usize) -> Option<(usize, usize)> {
    const PROT_READ: i32 = 1;
    const PROT_WRITE: i32 = 2;
    const MAP_PRIVATE: i32 = 0x02;
    const MAP_ANONYMOUS: i32 = 0x20;
    let hint = if target == 0 {
        std::ptr::null_mut()
    } else {
        target as *mut std::ffi::c_void
    };
    let ptr = unsafe {
        mmap(
            hint,
            len,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    if ptr as usize == usize::MAX || ptr.is_null() {
        None
    } else {
        Some((ptr as usize, len))
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn mmap(
        addr: *mut std::ffi::c_void,
        len: usize,
        prot: i32,
        flags: i32,
        fd: i32,
        offset: i64,
    ) -> *mut std::ffi::c_void;
    fn mprotect(addr: *mut std::ffi::c_void, len: usize, prot: i32) -> i32;
}

#[cfg(unix)]
fn make_executable(ptr: *mut u8, len: usize) -> Result<(), String> {
    const PROT_READ: i32 = 1;
    const PROT_EXEC: i32 = 4;
    if unsafe { mprotect(ptr as *mut _, len, PROT_READ | PROT_EXEC) } != 0 {
        return Err("mprotect failed".to_string());
    }
    Ok(())
}

pub fn resolve_bridge_lib_paths(dirs: &[PathBuf], names: &[String]) -> Vec<String> {
    let c_names: Vec<std::ffi::CString> = names
        .iter()
        .filter_map(|n| std::ffi::CString::new(n.as_str()).ok())
        .collect();
    let mut contributors: Vec<String> = Vec::new();
    for dir in dirs {
        let candidates = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for entry in candidates.flatten() {
            let path = entry.path();
            if !path.is_file() || !is_dylib_path(&path) {
                continue;
            }
            let handle = match load_library(&path) {
                Some(h) => h,
                None => continue,
            };
            let hits = c_names.iter().any(|c| lookup_symbol(handle, c).is_some());
            if hits {
                // Prefer the import library next to the DLL: MSVC's link.exe
                // reliably resolves symbols through a .lib, while direct DLL
                // inputs are not always honored by every linker flavor.
                let chosen = import_lib_for(&path).unwrap_or_else(|| path.display().to_string());
                let stem = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                // One entry per bridge crate (by stem): the same DLL may be
                // discoverable from several search dirs, and a duplicate DLL
                // input next to its .lib can break the MSVC link.
                let dup = contributors.iter().any(|c| {
                    std::path::Path::new(c)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_lowercase())
                        .map(|s| s == stem || s == format!("{}.dll", stem))
                        .unwrap_or(false)
                });
                if !dup && !contributors.contains(&chosen) {
                    contributors.push(chosen);
                }
            }
        }
    }
    contributors
}

/// v1.4.5: find the import library accompanying a bridge DLL, if any.
/// Checks `<stem>.dll.lib` (Cargo/cdylib layout) then `<stem>.lib`.
fn import_lib_for(dll: &Path) -> Option<String> {
    let dir = dll.parent()?;
    let stem = dll.file_stem()?;
    for candidate in [
        dir.join(format!("{}.dll.lib", stem.to_string_lossy())),
        dir.join(format!("{}.lib", stem.to_string_lossy())),
    ] {
        if candidate.is_file() {
            return Some(candidate.display().to_string());
        }
    }
    None
}

fn is_dylib_path(path: &Path) -> bool {
    path.extension()
        .map(|e| {
            let e = e.to_string_lossy().to_lowercase();
            if cfg!(target_os = "macos") {
                e == "dylib" || e == "so"
            } else if cfg!(windows) {
                e == "dll"
            } else {
                e == "so"
            }
        })
        .unwrap_or(false)
}

#[cfg(windows)]
fn load_library(path: &Path) -> Option<usize> {
    use std::os::windows::ffi::OsStrExt;
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
    if handle == 0 {
        None
    } else {
        Some(handle as usize)
    }
}

#[cfg(windows)]
fn lookup_symbol(handle: usize, name: &std::ffi::CString) -> Option<*const u8> {
    let proc_addr = unsafe { GetProcAddress(handle as isize, name.as_ptr() as *const u8) };
    if proc_addr.is_null() {
        None
    } else {
        Some(proc_addr as *const u8)
    }
}

#[cfg(windows)]
unsafe extern "system" {
    fn LoadLibraryW(lpfilename: *const u16) -> isize;
    fn GetProcAddress(hmodule: isize, lpprocname: *const u8) -> *mut core::ffi::c_void;
}

#[cfg(unix)]
fn load_library(path: &Path) -> Option<usize> {
    let bytes = path.as_os_str().as_encoded_bytes();
    let c_path = match std::ffi::CString::new(bytes.to_vec()) {
        Ok(c) => c,
        Err(_) => return None,
    };
    let handle = unsafe { dlopen(c_path.as_ptr(), RTLD_NOW | RTLD_LOCAL) };
    if handle.is_null() {
        None
    } else {
        Some(handle as usize)
    }
}

#[cfg(unix)]
fn lookup_symbol(handle: usize, name: &std::ffi::CString) -> Option<*const u8> {
    let sym = unsafe { dlsym(handle as *mut core::ffi::c_void, name.as_ptr()) };
    if sym.is_null() {
        None
    } else {
        Some(sym as *const u8)
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn dlopen(filename: *const std::os::raw::c_char, flags: i32) -> *mut core::ffi::c_void;
    fn dlsym(
        handle: *mut core::ffi::c_void,
        symbol: *const std::os::raw::c_char,
    ) -> *mut core::ffi::c_void;
}

#[cfg(unix)]
const RTLD_NOW: i32 = 2;
#[cfg(unix)]
const RTLD_LOCAL: i32 = 0;
