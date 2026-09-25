/* ============================================================================
 * Datara Universal Runtime (v1.5.0 Modular Umbrella)
 *
 * This file serves as the unified translation unit for single-file compilation
 * targets (e.g. Clang / LLVM AOT pipeline via compile_with_clang).
 *
 * For native builds, build.rs compiles the submodules individually into
 * datara_runtime.lib, enabling full object-level dead code elimination
 * (/OPT:REF and COMDAT selection) by the platform linker.
 * ============================================================================ */

#if defined(__APPLE__)
#ifndef _DARWIN_C_SOURCE
#define _DARWIN_C_SOURCE 1
#endif
#else
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#ifndef _DEFAULT_SOURCE
#define _DEFAULT_SOURCE
#endif
#ifndef _POSIX_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#endif
#endif

#include "datara_runtime.h"
#include "datara_rt_internal.h"

#include "datara_alloc.c"
#include "datara_core.c"
#include "datara_crypto.c"
#include "datara_fs.c"
#include "datara_guard.c"
#include "datara_list.c"
#include "datara_map.c"
#include "datara_math.c"
#include "datara_mem.c"
#include "datara_memprofile.c"
#include "datara_napi.c"
#include "datara_net.c"
#include "datara_parallel.c"
#include "datara_pgo.c"
#include "datara_polyglot.c"
#include "datara_print.c"
#include "datara_py.c"
#include "datara_js.c"
#include "datara_rt_scheduler.c"
#include "datara_simd.c"
#include "datara_string.c"
#include "datara_systems.c"
#include "datara_thread.c"
#include "datara_ui.c"
#include "datara_value.c"
