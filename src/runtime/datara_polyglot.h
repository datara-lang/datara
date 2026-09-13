#ifndef DATARA_POLYGLOT_H
#define DATARA_POLYGLOT_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// ============================================================================
// Datara v1.3.0 Universal Polyglot Zero-Latency Engine
// ============================================================================

// Zig Zero-Latency Interop
int64_t datara_zig_eval_int(const char* code);
int64_t datara_zig_call(const char* symbol, int64_t arg);

// C# / .NET NativeAOT Direct C-ABI Invocations
int64_t datara_csharp_invoke_i64(const char* lib_name, const char* method_name, int64_t arg);
double  datara_csharp_invoke_f64(const char* lib_name, const char* method_name, double arg);

// Lua / LuaJIT In-Process Direct Stack Interop
int64_t datara_lua_eval_int(const char* code);
double  datara_lua_eval_float(const char* code);
int64_t datara_lua_exec(const char* code);

// Microsecond Parallel Polyglot Test Scheduler
int64_t datara_polyglot_parallel_exec(const char* engine_type, const char* code);

#ifdef __cplusplus
}
#endif

#endif // DATARA_POLYGLOT_H
