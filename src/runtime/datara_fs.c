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

#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <math.h>
#include <errno.h>
#include "datara_runtime.h"
#include "datara_rt_internal.h"

#ifdef _WIN32
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#include <winsock2.h>
#include <ws2tcpip.h>
#include <dbghelp.h>
#else
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/socket.h>
#include <sys/mman.h>
#ifndef MAP_ANONYMOUS
#ifdef MAP_ANON
#define MAP_ANONYMOUS MAP_ANON
#endif
#endif
#include <netinet/in.h>
#include <arpa/inet.h>
#include <netdb.h>
#include <unistd.h>
#include <fcntl.h>
#include <time.h>
#include <pthread.h>
#include <sched.h>
#include <dirent.h>
#if defined(__has_include)
#if __has_include(<execinfo.h>)
#include <execinfo.h>
#define DATARA_HAS_EXECINFO 1
#endif
#elif !defined(__musl__)
#include <execinfo.h>
#define DATARA_HAS_EXECINFO 1
#endif
#endif

#ifndef INADDR_NONE
#define INADDR_NONE ((unsigned long)0xffffffff)
#endif
#ifdef _WIN32
#include <windows.h>
int64_t now_ms(void) {
    static LARGE_INTEGER freq = {0};
    if (freq.QuadPart == 0) {
        QueryPerformanceFrequency(&freq);
    }
    LARGE_INTEGER count;
    QueryPerformanceCounter(&count);
    return (int64_t)((count.QuadPart * 1000) / freq.QuadPart);
}
int64_t datara_rt_now_ms(void) {
    return now_ms();
}
int64_t datara_rt_now_unix_ms(void) {
    FILETIME ft;
    GetSystemTimeAsFileTime(&ft);
    ULARGE_INTEGER uli;
    uli.LowPart = ft.dwLowDateTime;
    uli.HighPart = ft.dwHighDateTime;
    return (int64_t)((uli.QuadPart - 116444736000000000ULL) / 10000ULL);
}
int64_t datara_rt_now_precise_ms(void) {
    static LARGE_INTEGER freq = {0};
    if (freq.QuadPart == 0) {
        QueryPerformanceFrequency(&freq);
    }
    LARGE_INTEGER count;
    QueryPerformanceCounter(&count);
    return (int64_t)((count.QuadPart * 1000000) / freq.QuadPart);
}
int64_t now_ns(void) {
    static LARGE_INTEGER freq = {0};
    if (freq.QuadPart == 0) {
        QueryPerformanceFrequency(&freq);
    }
    LARGE_INTEGER count;
    QueryPerformanceCounter(&count);
    int64_t sec = count.QuadPart / freq.QuadPart;
    int64_t rem = count.QuadPart % freq.QuadPart;
    return sec * 1000000000LL + (rem * 1000000000LL) / freq.QuadPart;
}
int64_t datara_rt_now_ns(void) {
    return now_ns();
}
#else
#include <time.h>
int64_t now_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (int64_t)(ts.tv_sec * 1000 + ts.tv_nsec / 1000000);
}
int64_t datara_rt_now_ms(void) {
    return now_ms();
}
int64_t datara_rt_now_unix_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    return (int64_t)(ts.tv_sec * 1000 + ts.tv_nsec / 1000000);
}
int64_t datara_rt_now_precise_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (int64_t)ts.tv_sec * 1000000LL + (int64_t)(ts.tv_nsec / 1000LL);
}
int64_t now_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (int64_t)ts.tv_sec * 1000000000LL + ts.tv_nsec;
}
int64_t datara_rt_now_ns(void) {
    return now_ns();
}
#endif

static FILE* datara_rt_fopen(const char* path, const char* mode) {
    if (!path || !mode) return NULL;
#ifdef _WIN32
    int wpath_len = MultiByteToWideChar(CP_UTF8, 0, path, -1, NULL, 0);
    int wmode_len = MultiByteToWideChar(CP_UTF8, 0, mode, -1, NULL, 0);
    if (wpath_len > 0 && wmode_len > 0) {
        wchar_t* wpath = (wchar_t*)malloc((size_t)wpath_len * sizeof(wchar_t));
        wchar_t* wmode = (wchar_t*)malloc((size_t)wmode_len * sizeof(wchar_t));
        if (wpath && wmode) {
            MultiByteToWideChar(CP_UTF8, 0, path, -1, wpath, wpath_len);
            MultiByteToWideChar(CP_UTF8, 0, mode, -1, wmode, wmode_len);
            FILE* f = _wfopen(wpath, wmode);
            free(wpath);
            free(wmode);
            return f;
        }
        if (wpath) free(wpath);
        if (wmode) free(wmode);
    }
    return fopen(path, mode);
#else
    return fopen(path, mode);
#endif
}

int64_t datara_rt_file_write(const char* path, const char* content) {
    datara_rt_cap_require(DATARA_CAP_FS_WRITE, "fs::write");
    if (!path || !content) return 0;
    FILE* f = datara_rt_fopen(path, "wb");
    if (!f) return 0;
    size_t len = strlen(content);
    size_t written = fwrite(content, 1, len, f);
    fclose(f);
    return written == len ? 1 : 0;
}

int64_t datara_rt_file_append(const char* path, const char* content) {
    datara_rt_cap_require(DATARA_CAP_FS_WRITE, "fs::append");
    if (!path || !content) return 0;
    FILE* f = datara_rt_fopen(path, "ab");
    if (!f) return 0;
    size_t len = strlen(content);
    size_t written = fwrite(content, 1, len, f);
    fclose(f);
    return written == len ? 1 : 0;
}

const char* datara_rt_file_read(const char* path) {
    datara_rt_cap_require(DATARA_CAP_FS_READ, "fs::read");
    if (!path) return "";
    FILE* f = datara_rt_fopen(path, "rb");
    if (!f) return "";
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    if (sz < 0 || sz > 1024L * 1024 * 1024) { fclose(f); return ""; }
    fseek(f, 0, SEEK_SET);
    char* buf = (char*)malloc((size_t)sz + 1);
    if (!buf) { fclose(f); return ""; }
    size_t read_bytes = fread(buf, 1, (size_t)sz, f);
    buf[read_bytes] = '\0';
    fclose(f);
    return buf;
}

// Binary-safe file I/O. Datara strings are NUL-terminated C strings at the
// runtime ABI boundary, so a length channel does not exist for text reads:
// `datara_rt_file_read` keeps the whole file in memory (fread, NUL bytes
// included) but every strlen-based consumer stops at the first NUL. The
// bytes builtins carry the real length through the list ABI instead:
// each element is one byte value (0..=255), which round-trips binary
// content exactly.
int64_t* datara_rt_file_read_bytes(const char* path) {
    datara_rt_cap_require(DATARA_CAP_FS_READ, "fs::read_bytes");
    if (!path) return datara_rt_list_create(0);
    FILE* f = datara_rt_fopen(path, "rb");
    if (!f) return datara_rt_list_create(0);
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    if (sz < 0 || sz > 1024L * 1024 * 1024) { fclose(f); return datara_rt_list_create(0); }
    fseek(f, 0, SEEK_SET);
    char* buf = (char*)malloc((size_t)sz > 0 ? (size_t)sz : 1);
    if (!buf) { fclose(f); return datara_rt_list_create(0); }
    size_t read_bytes = fread(buf, 1, (size_t)sz, f);
    fclose(f);
    int64_t* list = datara_rt_list_create_capacity((int64_t)read_bytes);
    if (!list) { free(buf); return datara_rt_list_create(0); }
    for (size_t i = 0; i < read_bytes; i++) {
        list = datara_rt_list_append(list, (int64_t)(unsigned char)buf[i]);
    }
    free(buf);
    return list;
}

int64_t datara_rt_file_write_bytes(const char* path, const int64_t* bytes) {
    datara_rt_cap_require(DATARA_CAP_FS_WRITE, "fs::write_bytes");
    if (!path) return 0;
    FILE* f = datara_rt_fopen(path, "wb");
    if (!f) return 0;
    int64_t count = bytes ? datara_rt_list_len((int64_t*)bytes) : 0;
    size_t written = 0;
    for (int64_t i = 0; i < count; i++) {
        unsigned char b = (unsigned char)(bytes[i + 1] & 0xFF);
        written += fwrite(&b, 1, 1, f);
    }
    fclose(f);
    return written == (size_t)count ? 1 : 0;
}

int64_t datara_rt_file_exists(const char* path) {
    datara_rt_cap_require(DATARA_CAP_FS_READ, "fs::exists");
    if (!path) return 0;
    FILE* f = datara_rt_fopen(path, "rb");
    if (f) {
        fclose(f);
        return 1;
    }
    return 0;
}

// ---------------------------------------------------------------------------
// Checked I/O returning Outcome<Str> objects.
//
// The layout mirrors the stdlib `Outcome<T>` class field-for-field, so an
// object built here is indistinguishable from an `Outcome<T> { ... }` struct
// literal lowered by the compiler (three 8-byte slots, field i at offset
// i*8): [0] = is_success (Bool as i64), [1] = value (char* payload, 0 when
// err), [2] = error_msg (char*, "" when ok). Like every other runtime
// string/object return, the allocation is intentionally never freed.
// ---------------------------------------------------------------------------
void* datara_rt_outcome_build(int64_t is_success, const char* value, const char* msg) {
    int64_t* obj = (int64_t*)malloc(3 * sizeof(int64_t));
    if (!obj) return NULL;
    obj[0] = is_success;
    obj[1] = (int64_t)(uintptr_t)(value ? value : "");
    obj[2] = (int64_t)(uintptr_t)(msg ? msg : "");
    return (void*)obj;
}

// Bit-exact value variant of datara_rt_outcome_build. The string-oriented
// builder substitutes "" for a NULL value, which would corrupt an integer 0
// payload or an all-zero float bit pattern into a bogus pointer. List
// element payloads are raw i64 slots (Int values, Float bit patterns, Str
// pointers), so they must be stored verbatim.
void* datara_rt_outcome_build_raw(int64_t is_success, int64_t value_raw, const char* msg) {
    int64_t* obj = (int64_t*)malloc(3 * sizeof(int64_t));
    if (!obj) return NULL;
    obj[0] = is_success;
    obj[1] = value_raw;
    obj[2] = (int64_t)(uintptr_t)(msg ? msg : "");
    return (void*)obj;
}

// Checked list accessors shared by first/last/pop: an empty list yields a
// failed Outcome with the fixed "empty list" error, a non-empty one a
// successful Outcome whose value slot carries the element bit-exactly.
// String/class elements are copied as pointers (never-free invariant: no
// refcounting, nothing is ever released).
static void* datara_rt_list_checked_elem(int64_t* list, int64_t idx) {
    if (!list || list[0] <= 0) {
        return datara_rt_outcome_build_raw(0, 0, "empty list");
    }
    if (idx < 0) {
        idx = list[0] - 1;
    } else if (idx >= list[0]) {
        idx = 0;
    }
    return datara_rt_outcome_build_raw(1, list[idx + 1], "");
}

// first: Outcome<T>, failed ("empty list") when the list is empty.
void* datara_rt_list_first(int64_t* list) {
    return datara_rt_list_checked_elem(list, 0);
}

// last: Outcome<T>, failed ("empty list") when the list is empty.
void* datara_rt_list_last(int64_t* list) {
    return datara_rt_list_checked_elem(list, -1);
}

// v1.4.1 checked pop: removes and returns the last element as Outcome<T>.
// Empty list -> is_success = 0, error_msg = "empty list"; the legacy
// bare-element pop survives as datara_rt_list_pop_legacy.
void* datara_rt_list_pop_outcome(int64_t* list) {
    if (!list || list[0] <= 0) {
        return datara_rt_outcome_build_raw(0, 0, "empty list");
    }
    int64_t count = list[0];
    int64_t val = list[count];
    list[0] = count - 1;
    return datara_rt_outcome_build_raw(1, val, "");
}

// v1.4.5 W1: checked arithmetic builtins. `bits` selects the operand
// width (8/16/32/64); both operands and the payload ride in I64 slots.
// Overflow is detected at the *operand* width, not i64, so Int8 arithmetic
// reports the Int8 overflow. The result is an Outcome<IntN> object in the
// stdlib Outcome<T> layout: ok carries the raw payload, err carries the
// diagnostic message. Narrowing on the ok path is the caller's business
// (the checker knows the payload type and unwrapping narrows).
static void* datara_rt_checked_op_outcome(int bits, int64_t a, int64_t b, int is_sub, int is_mul) {
    int64_t res = 0;
    int ovf = 0;
    const char* opname = is_sub ? "subtraction" : (is_mul ? "multiplication" : "addition");
#if defined(__GNUC__) || defined(__clang__)
    if (is_mul) {
        if (bits == 8)  ovf = __builtin_mul_overflow((int8_t)a,  (int8_t)b,  (int8_t*)&res);
        else if (bits == 16) ovf = __builtin_mul_overflow((int16_t)a, (int16_t)b, (int16_t*)&res);
        else if (bits == 32) ovf = __builtin_mul_overflow((int32_t)a, (int32_t)b, (int32_t*)&res);
        else ovf = __builtin_mul_overflow(a, b, &res);
    } else if (is_sub) {
        if (bits == 8)  ovf = __builtin_sub_overflow((int8_t)a,  (int8_t)b,  (int8_t*)&res);
        else if (bits == 16) ovf = __builtin_sub_overflow((int16_t)a, (int16_t)b, (int16_t*)&res);
        else if (bits == 32) ovf = __builtin_sub_overflow((int32_t)a, (int32_t)b, (int32_t*)&res);
        else ovf = __builtin_sub_overflow(a, b, &res);
    } else {
        if (bits == 8)  ovf = __builtin_add_overflow((int8_t)a,  (int8_t)b,  (int8_t*)&res);
        else if (bits == 16) ovf = __builtin_add_overflow((int16_t)a, (int16_t)b, (int16_t*)&res);
        else if (bits == 32) ovf = __builtin_add_overflow((int32_t)a, (int32_t)b, (int32_t*)&res);
        else ovf = __builtin_add_overflow(a, b, &res);
    }
#else
    // MSVC fallback: signed overflow at each width, computed in i64 with
    // explicit range checks (no UB: the widened multiply/add never overflows).
    if (is_mul) {
        if (bits == 8) {
            int64_t prod = (int64_t)(int8_t)a * (int64_t)(int8_t)b;
            ovf = prod < INT8_MIN || prod > INT8_MAX;
            res = (int8_t)prod;
        } else if (bits == 16) {
            int64_t prod = (int64_t)(int16_t)a * (int64_t)(int16_t)b;
            ovf = prod < INT16_MIN || prod > INT16_MAX;
            res = (int16_t)prod;
        } else if (bits == 32) {
            int64_t prod = (int64_t)(int32_t)a * (int64_t)(int32_t)b;
            ovf = prod < INT32_MIN || prod > INT32_MAX;
            res = (int32_t)prod;
        } else {
            ovf = (b != 0) && ((a > INT64_MAX / b) || (a < INT64_MIN / b))
                  && !(a == -1 && b == INT64_MIN) && !(b == -1 && a == INT64_MIN);
            res = a * b;
        }
    } else if (is_sub) {
        if (bits == 8) {
            int64_t diff = (int8_t)a - (int64_t)(int8_t)b;
            ovf = diff < INT8_MIN || diff > INT8_MAX;
            res = (int8_t)diff;
        } else if (bits == 16) {
            int64_t diff = (int16_t)a - (int64_t)(int16_t)b;
            ovf = diff < INT16_MIN || diff > INT16_MAX;
            res = (int16_t)diff;
        } else if (bits == 32) {
            int64_t diff = (int32_t)a - (int64_t)(int32_t)b;
            ovf = diff < INT32_MIN || diff > INT32_MAX;
            res = (int32_t)diff;
        } else {
            ovf = (b < 0 && a > INT64_MAX + b) || (b > 0 && a < INT64_MIN + b);
            res = a - b;
        }
    } else {
        if (bits == 8) {
            int64_t sum = (int8_t)a + (int64_t)(int8_t)b;
            ovf = sum < INT8_MIN || sum > INT8_MAX;
            res = (int8_t)sum;
        } else if (bits == 16) {
            int64_t sum = (int16_t)a + (int64_t)(int16_t)b;
            ovf = sum < INT16_MIN || sum > INT16_MAX;
            res = (int16_t)sum;
        } else if (bits == 32) {
            int64_t sum = (int32_t)a + (int64_t)(int32_t)b;
            ovf = sum < INT32_MIN || sum > INT32_MAX;
            res = (int32_t)sum;
        } else {
            ovf = (b > 0 && a > INT64_MAX - b) || (b < 0 && a < INT64_MIN - b);
            res = a + b;
        }
    }
#endif
    if (ovf) {
        char stack_msg[96];
        snprintf(stack_msg, sizeof(stack_msg),
                 "overflow in Int%d %s: %lld %s %lld",
                 (int)bits, opname, (long long)a, is_mul ? "*" : (is_sub ? "-" : "+"), (long long)b);
        char* msg = datara_str_scratch_alloc(strlen(stack_msg) + 1);
        if (msg) memcpy(msg, stack_msg, strlen(stack_msg) + 1);
        return datara_rt_outcome_build_raw(0, 0, msg ? msg : "integer overflow");
    }
    return datara_rt_outcome_build_raw(1, res, "");
}

void* datara_rt_checked_add_outcome(int bits, int64_t a, int64_t b) {
    return datara_rt_checked_op_outcome(bits, a, b, 0, 0);
}

void* datara_rt_checked_sub_outcome(int bits, int64_t a, int64_t b) {
    return datara_rt_checked_op_outcome(bits, a, b, 1, 0);
}

void* datara_rt_checked_mul_outcome(int bits, int64_t a, int64_t b) {
    return datara_rt_checked_op_outcome(bits, a, b, 0, 1);
}

void* datara_rt_file_read_checked(const char* path) {
    datara_rt_cap_require(DATARA_CAP_FS_READ, "fs::read_checked");
    if (!path) {
        return datara_rt_outcome_build(0, "", "file read failed: path is null");
    }
    FILE* f = datara_rt_fopen(path, "rb");
    if (!f) {
        char stack_msg[512];
        snprintf(stack_msg, sizeof(stack_msg), "file read failed: cannot open '%s'", path);
        char* msg = datara_str_scratch_alloc(strlen(stack_msg) + 1);
        if (msg) memcpy(msg, stack_msg, strlen(stack_msg) + 1);
        return datara_rt_outcome_build(0, "", msg ? msg : "file read failed: cannot open file");
    }
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    if (sz < 0 || sz > 1024L * 1024 * 1024) {
        fclose(f);
        return datara_rt_outcome_build(0, "", "file read failed: file too large or unreadable");
    }
    fseek(f, 0, SEEK_SET);
    char* buf = (char*)malloc((size_t)sz + 1);
    if (!buf) {
        fclose(f);
        return datara_rt_outcome_build(0, "", "file read failed: out of memory");
    }
    size_t read_bytes = fread(buf, 1, (size_t)sz, f);
    buf[read_bytes] = '\0';
    fclose(f);
    return datara_rt_outcome_build(1, buf, "");
}

void* datara_rt_sys_caps_create(void) {
    return (void*)0xCAFE;
}

void* datara_rt_files_grant_readonly(void* prov, const char* path) {
    (void)prov;
    return (void*)path;
}

void* datara_rt_files_grant_readwrite(void* prov, const char* path) {
    (void)prov;
    return (void*)path;
}

void* datara_rt_net_grant_connect(void* prov, const char* host, int64_t port) {
    (void)prov;
    (void)host;
    (void)port;
    return (void*)0xCAFE;
}

void* datara_rt_file_open(void* token, const char* path) {
    (void)token;
    return (void*)path;
}

const char* datara_rt_file_read_all(void* handle) {
    if (!handle) return "";
    return datara_rt_file_read((const char*)handle);
}

int64_t datara_rt_file_close(void* handle) {
    (void)handle;
    return 0;
}

void datara_rt_sleep(int64_t ms) {
    if (ms <= 0) return;
#ifdef _WIN32
    Sleep((DWORD)ms);
#else
    struct timespec ts;
    ts.tv_sec = ms / 1000;
    ts.tv_nsec = (ms % 1000) * 1000000;
    nanosleep(&ts, NULL);
#endif
}

const char* datara_rt_env_get(const char* key) {
    datara_rt_cap_require(DATARA_CAP_SYS_ENV, "sys::env");
    if (!key) return "";
    const char* val = getenv(key);
    return val ? val : "";
}

int64_t datara_rt_env_set(const char* key, const char* value) {
    datara_rt_cap_require(DATARA_CAP_SYS_ENV, "sys::env_set");
    if (!key) return 0;
#ifdef _WIN32
    // Both layers must be updated: SetEnvironmentVariableA writes the
    // process environment block, but the CRT keeps its own cache that
    // getenv() reads — without _putenv a subsequent datara_rt_env_get
    // (plain getenv) silently returns the stale/missing value.
    int crt_ok = _putenv_s(key, value ? value : "");
    int proc_ok = SetEnvironmentVariableA(key, value) ? 1 : 0;
    return (crt_ok == 0 && proc_ok) ? 1 : 0;
#else
    if (!value) {
        return unsetenv(key) == 0 ? 1 : 0;
    }
    return setenv(key, value, 1) == 0 ? 1 : 0;
#endif
}

void* datara_rt_env_get_checked(const char* name) {
    datara_rt_cap_require(DATARA_CAP_SYS_ENV, "sys::env_checked");
    if (!name) {
        return datara_rt_outcome_build(0, "", "env get failed: name is null");
    }
    const char* val = getenv(name);
    if (!val) {
        char stack_msg[512];
        snprintf(stack_msg, sizeof(stack_msg), "env get failed: '%s' is not set", name);
        char* msg = datara_str_scratch_alloc(strlen(stack_msg) + 1);
        if (msg) memcpy(msg, stack_msg, strlen(stack_msg) + 1);
        return datara_rt_outcome_build(0, "", msg ? msg : "env get failed: variable is not set");
    }
    return datara_rt_outcome_build(1, val, "");
}

// Directory listing. Returns a Datara List (list[0] = length, elements at
// list[1..]) whose elements are NUL-terminated entry names sorted with
// strcmp byte order. "." and ".." are skipped. Entry name buffers are
// malloc'd and intentionally never freed, matching every other runtime
// string return.
static int datara_rt_dir_name_cmp(const void* a, const void* b) {
    const char* sa = *(const char* const*)a;
    const char* sb = *(const char* const*)b;
    return strcmp(sa, sb);
}

int64_t* datara_rt_dir_list(const char* path) {
    datara_rt_cap_require(DATARA_CAP_FS_READ, "fs::dir_list");
    if (!path) return datara_rt_list_create(0);

    size_t plen = strlen(path);
    char pattern[1024];
#ifdef _WIN32
    int has_sep = plen > 0 && (path[plen - 1] == '/' || path[plen - 1] == '\\');
    if (has_sep) {
        snprintf(pattern, sizeof(pattern), "%s*", path);
    } else {
        snprintf(pattern, sizeof(pattern), "%s\\*", path);
    }
#else
    (void)plen;
    pattern[0] = '\0';
#endif

    // Collect entry names into a growable malloc'd array first so the
    // final list can be created with the exact capacity and the names
    // sorted before insertion.
    size_t count = 0;
    size_t cap = 16;
    char** names = (char**)malloc(cap * sizeof(char*));
    if (!names) return datara_rt_list_create(0);

#ifdef _WIN32
    int wpat_len = MultiByteToWideChar(CP_UTF8, 0, pattern, -1, NULL, 0);
    wchar_t* wpattern = (wpat_len > 0) ? (wchar_t*)malloc((size_t)wpat_len * sizeof(wchar_t)) : NULL;
    if (wpattern) MultiByteToWideChar(CP_UTF8, 0, pattern, -1, wpattern, wpat_len);
    WIN32_FIND_DATAW fd;
    HANDLE h = wpattern ? FindFirstFileW(wpattern, &fd) : INVALID_HANDLE_VALUE;
    if (wpattern) free(wpattern);
    if (h == INVALID_HANDLE_VALUE) {
        free(names);
        return datara_rt_list_create(0);
    }
    do {
        if (wcscmp(fd.cFileName, L".") == 0 || wcscmp(fd.cFileName, L"..") == 0) continue;
        if (count == cap) {
            size_t ncap = cap * 2;
            char** nn = (char**)realloc(names, ncap * sizeof(char*));
            if (!nn) break;
            names = nn;
            cap = ncap;
        }
        int u8_len = WideCharToMultiByte(CP_UTF8, 0, fd.cFileName, -1, NULL, 0, NULL, NULL);
        if (u8_len <= 0) continue;
        char* copy = (char*)malloc((size_t)u8_len);
        if (!copy) break;
        WideCharToMultiByte(CP_UTF8, 0, fd.cFileName, -1, copy, u8_len, NULL, NULL);
        names[count++] = copy;
    } while (FindNextFileW(h, &fd));
    FindClose(h);
#else
    DIR* d = opendir(path);
    if (!d) {
        free(names);
        return datara_rt_list_create(0);
    }
    struct dirent* ent;
    while ((ent = readdir(d)) != NULL) {
        const char* name = ent->d_name;
        if (strcmp(name, ".") == 0 || strcmp(name, "..") == 0) continue;
        if (count == cap) {
            size_t ncap = cap * 2;
            char** nn = (char**)realloc(names, ncap * sizeof(char*));
            if (!nn) break;
            names = nn;
            cap = ncap;
        }
        size_t nlen = strlen(name);
        char* copy = (char*)malloc(nlen + 1);
        if (!copy) break;
        memcpy(copy, name, nlen + 1);
        names[count++] = copy;
    }
    closedir(d);
#endif

    qsort(names, count, sizeof(char*), datara_rt_dir_name_cmp);

    int64_t* list = datara_rt_list_create_capacity((int64_t)count);
    if (!list) {
        free(names);
        return datara_rt_list_create(0);
    }
    for (size_t i = 0; i < count; i++) {
        list = datara_rt_list_append(list, (int64_t)(uintptr_t)names[i]);
    }
    free(names);
    return list;
}

// Existence probe that accepts both files and directories (unlike
// datara_rt_file_exists, whose fopen probe cannot open directories on
// Windows).
int64_t datara_rt_path_exists(const char* path) {
    datara_rt_cap_require(DATARA_CAP_FS_READ, "fs::path_exists");
    if (!path) return 0;
#ifdef _WIN32
    int wlen = MultiByteToWideChar(CP_UTF8, 0, path, -1, NULL, 0);
    if (wlen <= 0) return 0;
    wchar_t* wpath = (wchar_t*)malloc((size_t)wlen * sizeof(wchar_t));
    if (!wpath) return 0;
    MultiByteToWideChar(CP_UTF8, 0, path, -1, wpath, wlen);
    DWORD attrs = GetFileAttributesW(wpath);
    free(wpath);
    return attrs != INVALID_FILE_ATTRIBUTES ? 1 : 0;
#else
    struct stat st;
    return stat(path, &st) == 0 ? 1 : 0;
#endif
}

const char* datara_rt_path_join(const char* a, const char* b) {
    if (!a && !b) return "";
    if (!a || strlen(a) == 0) return b ? b : "";
    if (!b || strlen(b) == 0) return a ? a : "";
    size_t la = strlen(a);
    size_t lb = strlen(b);
    int needs_sep = (a[la - 1] != '/' && a[la - 1] != '\\' && b[0] != '/' && b[0] != '\\');
    size_t total = la + (needs_sep ? 1 : 0) + lb + 1;
    char* buf = datara_str_scratch_alloc(total);
    if (!buf) return "";
    memcpy(buf, a, la);
    size_t pos = la;
    if (needs_sep) {
#ifdef _WIN32
        buf[pos++] = '\\';
#else
        buf[pos++] = '/';
#endif
    }
    memcpy(buf + pos, b, lb);
    buf[pos + lb] = '\0';
    return buf;
}


int64_t datara_rt_str_contains(const char* s, const char* sub) {
    if (!s || !sub) return 0;
    return strstr(s, sub) != NULL ? 1 : 0;
}

int64_t datara_rt_str_starts_with(const char* s, const char* prefix) {
    if (!s || !prefix) return 0;
    size_t len_s = strlen(s);
    size_t len_p = strlen(prefix);
    if (len_s < len_p) return 0;
    return strncmp(s, prefix, len_p) == 0 ? 1 : 0;
}

int64_t datara_rt_str_ends_with(const char* s, const char* suffix) {
    if (!s || !suffix) return 0;
    size_t len_s = strlen(s);
    size_t len_suf = strlen(suffix);
    if (len_s < len_suf) return 0;
    return strcmp(s + len_s - len_suf, suffix) == 0 ? 1 : 0;
}

int64_t datara_rt_str_index_of(const char* s, const char* sub) {
    if (!s || !sub) return -1;
    const char* p = strstr(s, sub);
    if (!p) return -1;
    return (int64_t)(p - s);
}

const char* datara_rt_str_trim(const char* s) {
    if (!s) return "";
    while (*s == ' ' || *s == '\t' || *s == '\r' || *s == '\n') {
        s++;
    }
    if (*s == '\0') return "";
    size_t len = strlen(s);
    while (len > 0 && (s[len - 1] == ' ' || s[len - 1] == '\t' || s[len - 1] == '\r' || s[len - 1] == '\n')) {
        len--;
    }
    char* buf = (char*)malloc(len + 1);
    if (!buf) return "";
    memcpy(buf, s, len);
    buf[len] = '\0';
    return buf;
}

int64_t datara_rt_str_to_int(const char* s) {
    if (!s) return 0;
    return (int64_t)atoll(s);
}

double datara_rt_str_to_float(const char* s) {
    if (!s) return 0.0;
    return atof(s);
}

// M1 checked parsing: parse_int/parse_float return an Outcome object
// (is_success, raw payload, message) built with the bit-exact raw builder,
// so an integer 0 or an all-zero float bit pattern survives verbatim.
// Trailing garbage after the number is a parse error, not silent success.
static int rt_is_ws(char c) {
    return c == ' ' || (c >= 9 && c <= 13);
}

void* datara_rt_str_to_int_checked(const char* s) {
    if (!s) {
        return datara_rt_outcome_build_raw(0, 0, "parse_int: null string");
    }
    while (rt_is_ws(*s)) s++;
    if (*s == 0) {
        return datara_rt_outcome_build_raw(0, 0, "parse_int: empty string");
    }
    errno = 0;
    char* end = NULL;
    long long v = strtoll(s, &end, 10);
    if (end == s) {
        return datara_rt_outcome_build_raw(0, 0, "parse_int: not a number");
    }
    while (rt_is_ws(*end)) end++;
    if (*end != 0) {
        return datara_rt_outcome_build_raw(0, 0, "parse_int: trailing characters");
    }
    if (errno == ERANGE) {
        return datara_rt_outcome_build_raw(0, 0, "parse_int: number out of range");
    }
    return datara_rt_outcome_build_raw(1, (int64_t)v, "");
}

void* datara_rt_str_to_float_checked(const char* s) {
    if (!s) {
        return datara_rt_outcome_build_raw(0, 0, "parse_float: null string");
    }
    while (rt_is_ws(*s)) s++;
    if (*s == 0) {
        return datara_rt_outcome_build_raw(0, 0, "parse_float: empty string");
    }
    errno = 0;
    char* end = NULL;
    double v = strtod(s, &end);
    if (end == s) {
        return datara_rt_outcome_build_raw(0, 0, "parse_float: not a number");
    }
    while (rt_is_ws(*end)) end++;
    if (*end != 0) {
        return datara_rt_outcome_build_raw(0, 0, "parse_float: trailing characters");
    }
    if (errno == ERANGE) {
        return datara_rt_outcome_build_raw(0, 0, "parse_float: number out of range");
    }
    int64_t bits;
    memcpy(&bits, &v, sizeof(bits));
    return datara_rt_outcome_build_raw(1, bits, "");
}

// M1 UTF-8 codepoint indexing: translate a codepoint index to a byte
// offset (clamped to the string length). Non-characters (continuation
// bytes) never start a codepoint, so a codepoint index always lands on a
// lead byte.
int64_t datara_rt_str_cp_to_byte(const char* s, int64_t cp_index) {
    if (!s || cp_index <= 0) return 0;
    int64_t count = 0;
    const unsigned char* p = (const unsigned char*)s;
    while (*p) {
        if ((*p & 0xC0) != 0x80) {
            if (count == cp_index) {
                return (int64_t)(p - (const unsigned char*)s);
            }
            count++;
        }
        p++;
    }
    return (int64_t)(p - (const unsigned char*)s);
}

// Inverse mapping: byte offset -> how many codepoints precede it. Used to
// report find() results as codepoint indices.
int64_t datara_rt_str_byte_to_cp(const char* s, int64_t byte_off) {
    if (!s || byte_off <= 0) return 0;
    int64_t count = 0;
    const unsigned char* p = (const unsigned char*)s;
    const unsigned char* limit = p + byte_off;
    while (p < limit && *p) {
        if ((*p & 0xC0) != 0x80) {
            count++;
        }
        p++;
    }
    return count;
}

// Codepoint-indexed slice: [cp_start, cp_start+cp_len) clamped.
const char* datara_rt_str_substring_cp(const char* s, int64_t cp_start, int64_t cp_len) {
    if (!s || cp_start < 0 || cp_len <= 0) return "";
    int64_t byte_start = datara_rt_str_cp_to_byte(s, cp_start);
    if (s[byte_start] == 0) return "";
    const unsigned char* p = (const unsigned char*)s + byte_start;
    int64_t taken = 0;
    while (*p) {
        if ((*p & 0xC0) != 0x80) {
            // A lead byte starts the NEXT codepoint: once cp_len codepoints
            // are inside the slice, stop here. Continuation bytes of the
            // last included codepoint fall through the loop untouched, so
            // the cut always lands on a codepoint boundary.
            if (taken == cp_len) break;
            taken++;
        }
        p++;
    }
    int64_t byte_len = (int64_t)(p - (const unsigned char*)s) - byte_start;
    if (byte_len <= 0) return "";
    char* buf = datara_str_scratch_alloc((size_t)byte_len);
    if (!buf) return "";
    memcpy(buf, s + byte_start, (size_t)byte_len);
    buf[byte_len] = 0;
    return buf;
}

const char* datara_rt_str_substring(const char* s, int64_t start, int64_t len) {
    if (!s || start < 0 || len <= 0) return "";
    int64_t total_len = (int64_t)strlen(s);
    if (start >= total_len) return "";
    if (start + len > total_len) {
        len = total_len - start;
    }
    char* buf = datara_str_scratch_alloc((size_t)len);
    if (!buf) return "";
    memcpy(buf, s + start, (size_t)len);
    buf[len] = '\0';
    return buf;
}

int64_t datara_rt_str_chars(const char* s) {
    if (!s) return 0;
    int64_t count = 0;
    const unsigned char* p = (const unsigned char*)s;
    while (*p) {
        if ((*p & 0xC0) != 0x80) {
            count++;
        }
        p++;
    }
    return count;
}

int32_t datara_rt_validate_utf8(const char* s) {
    if (!s) return 1;
    const unsigned char* bytes = (const unsigned char*)s;
    size_t i = 0;
    size_t len = strlen(s);
    while (i < len) {
        unsigned char b0 = bytes[i];
        if (b0 <= 0x7F) {
            i += 1;
        } else if (b0 >= 0xC2 && b0 <= 0xDF) {
            if (i + 1 >= len) return 0;
            unsigned char b1 = bytes[i + 1];
            if ((b1 & 0xC0) != 0x80) return 0;
            i += 2;
        } else if (b0 == 0xE0) {
            if (i + 2 >= len) return 0;
            unsigned char b1 = bytes[i + 1];
            unsigned char b2 = bytes[i + 2];
            if (b1 < 0xA0 || b1 > 0xBF || (b2 & 0xC0) != 0x80) return 0;
            i += 3;
        } else if (b0 >= 0xE1 && b0 <= 0xEC) {
            if (i + 2 >= len) return 0;
            unsigned char b1 = bytes[i + 1];
            unsigned char b2 = bytes[i + 2];
            if ((b1 & 0xC0) != 0x80 || (b2 & 0xC0) != 0x80) return 0;
            i += 3;
        } else if (b0 == 0xED) {
            if (i + 2 >= len) return 0;
            unsigned char b1 = bytes[i + 1];
            unsigned char b2 = bytes[i + 2];
            if (b1 < 0x80 || b1 > 0x9F || (b2 & 0xC0) != 0x80) return 0;
            i += 3;
        } else if (b0 >= 0xEE && b0 <= 0xEF) {
            if (i + 2 >= len) return 0;
            unsigned char b1 = bytes[i + 1];
            unsigned char b2 = bytes[i + 2];
            if ((b1 & 0xC0) != 0x80 || (b2 & 0xC0) != 0x80) return 0;
            i += 3;
        } else if (b0 == 0xF0) {
            if (i + 3 >= len) return 0;
            unsigned char b1 = bytes[i + 1];
            unsigned char b2 = bytes[i + 2];
            unsigned char b3 = bytes[i + 3];
            if (b1 < 0x90 || b1 > 0xBF || (b2 & 0xC0) != 0x80 || (b3 & 0xC0) != 0x80) return 0;
            i += 4;
        } else if (b0 >= 0xF1 && b0 <= 0xF3) {
            if (i + 3 >= len) return 0;
            unsigned char b1 = bytes[i + 1];
            unsigned char b2 = bytes[i + 2];
            unsigned char b3 = bytes[i + 3];
            if ((b1 & 0xC0) != 0x80 || (b2 & 0xC0) != 0x80 || (b3 & 0xC0) != 0x80) return 0;
            i += 4;
        } else if (b0 == 0xF4) {
            if (i + 3 >= len) return 0;
            unsigned char b1 = bytes[i + 1];
            unsigned char b2 = bytes[i + 2];
            unsigned char b3 = bytes[i + 3];
            if (b1 < 0x80 || b1 > 0x8F || (b2 & 0xC0) != 0x80 || (b3 & 0xC0) != 0x80) return 0;
            i += 4;
        } else {
            return 0;
        }
    }
    return 1;
}

int64_t datara_rt_char_len(const char* s) {
    return datara_rt_str_chars(s);
}

const char* datara_rt_str_sanitize_utf8(const char* s) {
    if (!s) return "";
    if (datara_rt_validate_utf8(s)) return s;
    size_t len = strlen(s);
    char* buf = datara_str_scratch_alloc(len * 3 + 1);
    if (!buf) return "";
    size_t out_idx = 0;
    const unsigned char* p = (const unsigned char*)s;
    size_t i = 0;
    while (i < len) {
        unsigned char b0 = p[i];
        size_t seq_len = 0;
        if (b0 <= 0x7F) seq_len = 1;
        else if (b0 >= 0xC2 && b0 <= 0xDF && i + 1 < len && (p[i+1] & 0xC0) == 0x80) seq_len = 2;
        else if (b0 == 0xE0 && i + 2 < len && p[i+1] >= 0xA0 && p[i+1] <= 0xBF && (p[i+2] & 0xC0) == 0x80) seq_len = 3;
        else if (b0 >= 0xE1 && b0 <= 0xEC && i + 2 < len && (p[i+1] & 0xC0) == 0x80 && (p[i+2] & 0xC0) == 0x80) seq_len = 3;
        else if (b0 == 0xED && i + 2 < len && p[i+1] >= 0x80 && p[i+1] <= 0x9F && (p[i+2] & 0xC0) == 0x80) seq_len = 3;
        else if (b0 >= 0xEE && b0 <= 0xEF && i + 2 < len && (p[i+1] & 0xC0) == 0x80 && (p[i+2] & 0xC0) == 0x80) seq_len = 3;
        else if (b0 == 0xF0 && i + 3 < len && p[i+1] >= 0x90 && p[i+1] <= 0xBF && (p[i+2] & 0xC0) == 0x80 && (p[i+3] & 0xC0) == 0x80) seq_len = 4;
        else if (b0 >= 0xF1 && b0 <= 0xF3 && i + 3 < len && (p[i+1] & 0xC0) == 0x80 && (p[i+2] & 0xC0) == 0x80 && (p[i+3] & 0xC0) == 0x80) seq_len = 4;
        else if (b0 == 0xF4 && i + 3 < len && p[i+1] >= 0x80 && p[i+1] <= 0x8F && (p[i+2] & 0xC0) == 0x80 && (p[i+3] & 0xC0) == 0x80) seq_len = 4;

        if (seq_len > 0) {
            for (size_t k = 0; k < seq_len; k++) {
                buf[out_idx++] = (char)p[i + k];
            }
            i += seq_len;
        } else {
            buf[out_idx++] = (char)0xEF;
            buf[out_idx++] = (char)0xBF;
            buf[out_idx++] = (char)0xBD;
            i += 1;
        }
    }
    buf[out_idx] = '\0';
    return buf;
}

const char* datara_rt_str_next_scalar(const char* s, int64_t* inout_offset) {
    if (!s || !inout_offset || *inout_offset < 0) return "";
    size_t slen = strlen(s);
    if ((size_t)*inout_offset >= slen) return "";

    const unsigned char* p = (const unsigned char*)(s + *inout_offset);
    size_t char_len = 1;
    if (*p < 0x80) char_len = 1;
    else if ((*p & 0xE0) == 0xC0) char_len = 2;
    else if ((*p & 0xF0) == 0xE0) char_len = 3;
    else if ((*p & 0xF8) == 0xF0) char_len = 4;

    if ((size_t)*inout_offset + char_len > slen) {
        char_len = slen - (size_t)*inout_offset;
    }

    char* buf = datara_str_scratch_alloc(char_len);
    if (!buf) return "";
    memcpy(buf, p, char_len);
    buf[char_len] = '\0';
    *inout_offset += (int64_t)char_len;
    return buf;
}

const char* datara_rt_str_scalar_at(const char* s, int64_t offset) {
    if (!s || offset < 0) return "";
    size_t slen = strlen(s);
    if ((size_t)offset >= slen) return "";

    const unsigned char* p = (const unsigned char*)(s + offset);
    size_t char_len = 1;
    if (*p < 0x80) char_len = 1;
    else if ((*p & 0xE0) == 0xC0) char_len = 2;
    else if ((*p & 0xF0) == 0xE0) char_len = 3;
    else if ((*p & 0xF8) == 0xF0) char_len = 4;

    if ((size_t)offset + char_len > slen) {
        char_len = slen - (size_t)offset;
    }

    char* buf = datara_str_scratch_alloc(char_len);
    if (!buf) return "";
    memcpy(buf, p, char_len);
    buf[char_len] = '\0';
    return buf;
}

int64_t datara_rt_str_next_offset(const char* s, int64_t current_offset) {
    if (!s || current_offset < 0) return 0;
    size_t slen = strlen(s);
    if ((size_t)current_offset >= slen) return (int64_t)slen;

    const unsigned char* p = (const unsigned char*)(s + current_offset);
    size_t char_len = 1;
    if (*p < 0x80) char_len = 1;
    else if ((*p & 0xE0) == 0xC0) char_len = 2;
    else if ((*p & 0xF0) == 0xE0) char_len = 3;
    else if ((*p & 0xF8) == 0xF0) char_len = 4;

    if ((size_t)current_offset + char_len > slen) {
        char_len = slen - (size_t)current_offset;
    }
    return current_offset + (int64_t)char_len;
}

int64_t datara_rt_str_byte_at(const char* s, int64_t idx) {
    if (!s || idx < 0) return -1;
    size_t slen = strlen(s);
    if ((size_t)idx >= slen) return -1;
    return (int64_t)((const unsigned char*)s)[idx];
}

const char* datara_rt_str_char_at(const char* s, int64_t idx) {
    if (!s || idx < 0) return "";
    size_t slen = strlen(s);
    const unsigned char* p = (const unsigned char*)s;
    int64_t cur_char = 0;
    size_t byte_pos = 0;

    while (byte_pos < slen) {
        size_t char_len = 1;
        unsigned char b = p[byte_pos];
        if (b < 0x80) char_len = 1;
        else if ((b & 0xE0) == 0xC0) char_len = 2;
        else if ((b & 0xF0) == 0xE0) char_len = 3;
        else if ((b & 0xF8) == 0xF0) char_len = 4;

        if (byte_pos + char_len > slen) {
            char_len = slen - byte_pos;
        }

        if (cur_char == idx) {
            char* buf = datara_str_scratch_alloc(char_len);
            if (!buf) return "";
            memcpy(buf, s + byte_pos, char_len);
            buf[char_len] = '\0';
            return buf;
        }

        cur_char++;
        byte_pos += char_len;
    }
    return "";
}

const char* datara_rt_str_repeat(const char* s, int64_t count) {
    if (!s || count <= 0) return "";
    size_t slen = strlen(s);
    if (slen == 0) return "";
    if (count != 0 && slen > SIZE_MAX / (size_t)count) return NULL;
    size_t total = slen * (size_t)count;
    char* buf = datara_str_scratch_alloc(total);
    if (!buf) return "";
    char* p = buf;
    for (int64_t i = 0; i < count; i++) {
        memcpy(p, s, slen);
        p += slen;
    }
    *p = '\0';
    return buf;
}

const char* datara_rt_str_pad_left(const char* s, int64_t total_len, const char* pad) {
    if (!s) s = "";
    if (!pad || pad[0] == '\0') pad = " ";
    int64_t slen = (int64_t)strlen(s);
    if (slen >= total_len) return s;
    int64_t pad_needed = total_len - slen;
    size_t pad_len = strlen(pad);
    char* buf = datara_str_scratch_alloc((size_t)total_len);
    if (!buf) return s;
    char* p = buf;
    int64_t rem = pad_needed;
    while (rem > 0) {
        size_t take = (size_t)rem < pad_len ? (size_t)rem : pad_len;
        memcpy(p, pad, take);
        p += take;
        rem -= (int64_t)take;
    }
    memcpy(p, s, (size_t)slen);
    p += slen;
    *p = '\0';
    return buf;
}

const char* datara_rt_str_pad_right(const char* s, int64_t total_len, const char* pad) {
    if (!s) s = "";
    if (!pad || pad[0] == '\0') pad = " ";
    int64_t slen = (int64_t)strlen(s);
    if (slen >= total_len) return s;
    int64_t pad_needed = total_len - slen;
    size_t pad_len = strlen(pad);
    char* buf = datara_str_scratch_alloc((size_t)total_len);
    if (!buf) return s;
    char* p = buf;
    memcpy(p, s, (size_t)slen);
    p += slen;
    int64_t rem = pad_needed;
    while (rem > 0) {
        size_t take = (size_t)rem < pad_len ? (size_t)rem : pad_len;
        memcpy(p, pad, take);
        p += take;
        rem -= (int64_t)take;
    }
    *p = '\0';
    return buf;
}

const char* datara_rt_str_replace(const char* s, const char* target, const char* replacement) {
    if (!s) return "";
    if (!target || target[0] == '\0') return s;
    if (!replacement) replacement = "";
    size_t slen = strlen(s);
    size_t tlen = strlen(target);
    size_t rlen = strlen(replacement);

    size_t count = 0;
    const char* p = s;
    while ((p = strstr(p, target)) != NULL) {
        count++;
        p += tlen;
    }
    if (count == 0) return s;

    size_t new_len = slen + count * (rlen > tlen ? (rlen - tlen) : 0);
    char* buf = datara_str_scratch_alloc(new_len);
    if (!buf) return s;

    char* dst = buf;
    p = s;
    const char* match;
    while ((match = strstr(p, target)) != NULL) {
        size_t seg = (size_t)(match - p);
        memcpy(dst, p, seg);
        dst += seg;
        memcpy(dst, replacement, rlen);
        dst += rlen;
        p = match + tlen;
    }
    size_t rest = strlen(p);
    memcpy(dst, p, rest);
    dst[rest] = '\0';
    return buf;
}

const char* datara_rt_str_to_upper(const char* s) {
    if (!s) return "";
    size_t len = strlen(s);
    char* buf = datara_str_scratch_alloc(len);
    if (!buf) return "";
    for (size_t i = 0; i < len; i++) {
        char c = s[i];
        if (c >= 'a' && c <= 'z') c = (char)(c - ('a' - 'A'));
        buf[i] = c;
    }
    buf[len] = '\0';
    return buf;
}

const char* datara_rt_str_to_lower(const char* s) {
    if (!s) return "";
    size_t len = strlen(s);
    char* buf = datara_str_scratch_alloc(len);
    if (!buf) return "";
    for (size_t i = 0; i < len; i++) {
        char c = s[i];
        if (c >= 'A' && c <= 'Z') c = (char)(c + ('a' - 'A'));
        buf[i] = c;
    }
    buf[len] = '\0';
    return buf;
}

int64_t* datara_rt_str_split(const char* s, const char* delim) {
    if (!s) return datara_rt_list_create(0);
    if (!delim || delim[0] == '\0') {
        int64_t* list = datara_rt_list_create_capacity(1);
        list = datara_rt_list_append(list, (int64_t)s);
        return list;
    }
    size_t delim_len = strlen(delim);
    int64_t* list = datara_rt_list_create_capacity(8);
    const char* cur = s;
    const char* found;
    while ((found = strstr(cur, delim)) != NULL) {
        size_t part_len = (size_t)(found - cur);
        char* part = datara_str_scratch_alloc(part_len);
        if (part) {
            memcpy(part, cur, part_len);
            part[part_len] = '\0';
            list = datara_rt_list_append(list, (int64_t)part);
        }
        cur = found + delim_len;
    }
    size_t rem_len = strlen(cur);
    char* rem = datara_str_scratch_alloc(rem_len);
    if (rem) {
        memcpy(rem, cur, rem_len);
        rem[rem_len] = '\0';
        list = datara_rt_list_append(list, (int64_t)rem);
    }
    return list;
}

const char* datara_rt_str_join(const int64_t* list, const char* delim) {
    if (!list) return "";
    int64_t count = datara_rt_list_len((int64_t*)list);
    if (count == 0) return "";
    if (!delim) delim = "";
    size_t delim_len = strlen(delim);

    size_t total_len = 0;
    for (int64_t i = 0; i < count; i++) {
        const char* item = (const char*)datara_rt_list_get((int64_t*)list, i);
        if (item) {
            total_len += strlen(item);
        }
        if (i + 1 < count) {
            total_len += delim_len;
        }
    }

    char* res = datara_str_scratch_alloc(total_len);
    if (!res) return "";
    char* p = res;
    for (int64_t i = 0; i < count; i++) {
        const char* item = (const char*)datara_rt_list_get((int64_t*)list, i);
        if (item) {
            size_t ilen = strlen(item);
            memcpy(p, item, ilen);
            p += ilen;
        }
        if (i + 1 < count && delim_len > 0) {
            memcpy(p, delim, delim_len);
            p += delim_len;
        }
    }
    *p = '\0';
    return res;
}

const char* datara_rt_format_percent(double val, int64_t decimals) {
    if (decimals < 0) decimals = 0;
    if (decimals > 10) decimals = 10;
    char* buf = datara_str_scratch_alloc(32);
    if (!buf) return "";
    snprintf(buf, 32, "%.*f%%", (int)decimals, val * 100.0);
    return buf;
}

const char* datara_rt_format_int_with_commas(int64_t n) {
    char temp[32];
    snprintf(temp, sizeof(temp), "%lld", (long long)n);
    size_t len = strlen(temp);
    size_t digits_start = (temp[0] == '-') ? 1 : 0;
    size_t num_digits = len - digits_start;
    size_t commas = (num_digits > 0) ? (num_digits - 1) / 3 : 0;

    char* buf = datara_str_scratch_alloc(len + commas);
    if (!buf) return "";

    char* dst = buf;
    if (digits_start) {
        *dst++ = '-';
    }

    size_t first_group = num_digits % 3;
    if (first_group == 0) first_group = 3;

    const char* src = temp + digits_start;
    memcpy(dst, src, first_group);
    dst += first_group;
    src += first_group;

    while (*src) {
        *dst++ = ',';
        memcpy(dst, src, 3);
        dst += 3;
        src += 3;
    }
    *dst = '\0';
    return buf;
}

