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
// Ultra-Fast Zero-Allocation Streaming Terminal I/O Subsystem
#define DATARA_OUT_BUF_SIZE 16384
static DATARA_TLS char datara_out_buf[DATARA_OUT_BUF_SIZE];
static DATARA_TLS size_t datara_out_pos = 0;

static inline size_t datara_fast_i64toa(int64_t val, char* dst) {
    char temp[32];
    char* p = temp;
    uint64_t u = (uint64_t)val;
    size_t len = 0;
    if (val < 0) {
        dst[len++] = '-';
        u = ~u + 1;
    }
    do {
        *p++ = (char)('0' + (u % 10));
        u /= 10;
    } while (u > 0);
    while (p > temp) {
        dst[len++] = *--p;
    }
    return len;
}

void datara_rt_flush(void) {
    if (datara_out_pos > 0) {
#ifdef _WIN32
        HANDLE hOut = GetStdHandle(STD_OUTPUT_HANDLE);
        if (hOut != INVALID_HANDLE_VALUE && hOut != NULL) {
            DWORD written = 0;
            WriteFile(hOut, datara_out_buf, (DWORD)datara_out_pos, &written, NULL);
        } else {
            fwrite(datara_out_buf, 1, datara_out_pos, stdout);
            fflush(stdout);
        }
#else
        ssize_t ret = write(1, datara_out_buf, datara_out_pos);
        (void)ret;
#endif
        datara_out_pos = 0;
    }
}

static int g_flush_atexit_registered = 0;

#ifdef _WIN32
static inline int datara_rt_is_tty(void) {
    static int cached_is_tty = -1;
    if (cached_is_tty == -1) {
        HANDLE hOut = GetStdHandle(STD_OUTPUT_HANDLE);
        DWORD mode;
        cached_is_tty = (hOut != INVALID_HANDLE_VALUE && hOut != NULL && GetConsoleMode(hOut, &mode)) ? 1 : 0;
    }
    return cached_is_tty;
}
#else
#include <unistd.h>
static inline int datara_rt_is_tty(void) {
    static int cached_is_tty = -1;
    if (cached_is_tty == -1) {
        cached_is_tty = isatty(1) ? 1 : 0;
    }
    return cached_is_tty;
}
#endif

void datara_rt_flush_if_tty(void) {
    if (datara_rt_is_tty()) {
        datara_rt_flush();
    }
}

static DATARA_TLS int g_capture_enabled = 0;

static DATARA_TLS char* g_capture_buf = NULL;
static DATARA_TLS size_t g_capture_len = 0;
static DATARA_TLS size_t g_capture_cap = 0;
static DATARA_TLS char* g_capture_err_buf = NULL;
static DATARA_TLS size_t g_capture_err_len = 0;
static DATARA_TLS size_t g_capture_err_cap = 0;

void datara_rt_set_capture(int32_t enable) {
    g_capture_enabled = enable;
    if (enable) {
        datara_rt_clear_capture();
    }
}

void datara_rt_clear_capture(void) {
    if (g_capture_buf) {
        g_capture_buf[0] = '\0';
    }
    g_capture_len = 0;
    if (g_capture_err_buf) {
        g_capture_err_buf[0] = '\0';
    }
    g_capture_err_len = 0;
}

const char* datara_rt_get_capture(void) {
    datara_rt_flush();
    return g_capture_buf ? g_capture_buf : "";
}

const char* datara_rt_get_err_capture(void) {
    return g_capture_err_buf ? g_capture_err_buf : "";
}

static void datara_capture_append(const char* s, size_t len) {
    if (!s || len == 0) return;
    if (g_capture_len + len + 1 > g_capture_cap) {
        size_t new_cap = (g_capture_cap == 0) ? 4096 : (g_capture_cap * 2 + len + 1);
        char* new_buf = (char*)realloc(g_capture_buf, new_cap);
        if (!new_buf) return;
        g_capture_buf = new_buf;
        g_capture_cap = new_cap;
    }
    memcpy(g_capture_buf + g_capture_len, s, len);
    g_capture_len += len;
    g_capture_buf[g_capture_len] = '\0';
}

static void datara_capture_err_append(const char* s, size_t len) {
    if (!s || len == 0) return;
    if (g_capture_err_len + len + 1 > g_capture_err_cap) {
        size_t new_cap = (g_capture_err_cap == 0) ? 4096 : (g_capture_err_cap * 2 + len + 1);
        char* new_buf = (char*)realloc(g_capture_err_buf, new_cap);
        if (!new_buf) return;
        g_capture_err_buf = new_buf;
        g_capture_err_cap = new_cap;
    }
    memcpy(g_capture_err_buf + g_capture_err_len, s, len);
    g_capture_err_len += len;
    g_capture_err_buf[g_capture_err_len] = '\0';
}

static inline void datara_rt_err_buf_write(const char* s, size_t len) {
    if (!s || len == 0) return;
    if (g_capture_enabled) {
        datara_capture_err_append(s, len);
        return;
    }
    fwrite(s, 1, len, stderr);
}

static inline void datara_rt_buf_write(const char* s, size_t len) {
    if (!s || len == 0) return;
    if (!g_flush_atexit_registered) {
        atexit(datara_rt_flush);
        g_flush_atexit_registered = 1;
    }
    if (g_capture_enabled) {
        datara_capture_append(s, len);
        return;
    }
    if (len >= DATARA_OUT_BUF_SIZE) {
        datara_rt_flush();
#ifdef _WIN32
        HANDLE hOut = GetStdHandle(STD_OUTPUT_HANDLE);
        if (hOut != INVALID_HANDLE_VALUE && hOut != NULL) {
            DWORD written = 0;
            WriteFile(hOut, s, (DWORD)len, &written, NULL);
            return;
        }
#else
        ssize_t ret = write(1, s, len);
        (void)ret;
        return;
#endif
        fwrite(s, 1, len, stdout);
        fflush(stdout);
        return;
    }
    if (datara_out_pos + len > DATARA_OUT_BUF_SIZE) {
        datara_rt_flush();
    }
    memcpy(datara_out_buf + datara_out_pos, s, len);
    datara_out_pos += len;
}

void datara_rt_print_str(const char* s) {
    if (!s) {
        datara_rt_panic("invariant violation: null string pointer passed to 'print' statement");
    } else {
        datara_rt_buf_write(s, strlen(s));
    }
}

void datara_rt_print_int(int64_t v) {
    char buf[32];
    size_t len = datara_fast_i64toa(v, buf);
    datara_rt_buf_write(buf, len);
}

void datara_rt_print_float(double v) {
    if (isnan(v)) {
        datara_rt_buf_write(signbit(v) ? "-NaN" : "NaN", signbit(v) ? 4 : 3);
        return;
    }
    if (isinf(v)) {
        datara_rt_buf_write(v < 0 ? "-Infinity" : "Infinity", v < 0 ? 9 : 8);
        return;
    }
    char buf[64];
    if (v == floor(v) && fabs(v) < 1e15) {
        int len = snprintf(buf, sizeof(buf), "%.0f", v);
        datara_rt_buf_write(buf, (size_t)len);
        return;
    }
    for (int prec = 4; prec < 17; prec++) {
        snprintf(buf, sizeof(buf), "%.*g", prec, v);
        if (strtod(buf, NULL) == v) {
            datara_rt_buf_write(buf, strlen(buf));
            return;
        }
    }
    snprintf(buf, sizeof(buf), "%.17g", v);
    datara_rt_buf_write(buf, strlen(buf));
}

// v1.4.5 W2: Float32 printing. Shortest decimal that round-trips through the
// FLOAT (f32) type — `0.1f + 0.2f` prints `0.3`, not the f64 artifact.
// Placed after datara_rt_buf_write's definition (static inline above).
void datara_rt_print_f32(float v) {
    if (isnan(v)) {
        datara_rt_buf_write(signbit(v) ? "-NaN" : "NaN", signbit(v) ? 4 : 3);
        return;
    }
    if (isinf(v)) {
        datara_rt_buf_write(v < 0 ? "-Infinity" : "Infinity", v < 0 ? 9 : 8);
        return;
    }
    char buf[64];
    if (v == floorf(v) && fabsf(v) < 1e7f) {
        int len = snprintf(buf, sizeof(buf), "%.0f", (double)v);
        datara_rt_buf_write(buf, (size_t)len);
        return;
    }
    for (int prec = 4; prec < 10; prec++) {
        snprintf(buf, sizeof(buf), "%.*g", prec, (double)v);
        if (strtof(buf, NULL) == v) {
            datara_rt_buf_write(buf, strlen(buf));
            return;
        }
    }
    snprintf(buf, sizeof(buf), "%.9g", (double)v);
    datara_rt_buf_write(buf, strlen(buf));
}

void datara_rt_out_f32(float v) {
    datara_rt_print_f32(v);
    datara_rt_print_newline();
}

// v1.4.5 W3: Dec64 printing. The value is an i64 mantissa scaled by 10⁴;
// print integer part, a decimal point, and exactly 4 fractional digits.
// Trailing zeros in the fraction are trimmed for natural output (19.9900
// prints as 19.99) while exact fixed-point representation is preserved.
void datara_rt_print_dec64(int64_t val) {
    char buf[48];
    int64_t integer = val / 10000;
    int64_t frac = val % 10000;
    if (frac < 0) frac = -frac;
    if (val < 0 && integer == 0) {
        // Keep the sign visible for values in (-1, 0).
        int len = snprintf(buf, sizeof(buf), "-0.%04lld", (long long)frac);
        // Trim trailing zeros but keep at least one fractional digit.
        while (len > 3 && buf[len - 1] == '0') len--;
        datara_rt_buf_write(buf, (size_t)len);
        return;
    }
    int len = snprintf(buf, sizeof(buf), "%lld.%04lld", (long long)integer, (long long)frac);
    while (len > 0 && buf[len - 1] == '0') len--;
    if (len > 0 && buf[len - 1] == '.') len--;
    datara_rt_buf_write(buf, (size_t)len);
}

void datara_rt_out_dec64(int64_t val) {
    datara_rt_print_dec64(val);
    datara_rt_print_newline();
}

// v1.4.5 W3: Dec64 -> string for fmt"..." interpolation (FormatStr). Uses the
// shared TLS scratch allocator so the result outlives the call like the
// other *_to_str helpers; reuses the same formatting as datara_rt_print_dec64
// (4 fractional digits, trailing zeros trimmed).
const char* datara_rt_dec_to_str(int64_t val) {
    char* buf = datara_str_scratch_alloc(48);
    if (!buf) return "";
    int64_t integer = val / 10000;
    int64_t frac = val % 10000;
    if (frac < 0) frac = -frac;
    int len;
    if (val < 0 && integer == 0) {
        len = snprintf(buf, 48, "-0.%04lld", (long long)frac);
    } else {
        len = snprintf(buf, 48, "%lld.%04lld", (long long)integer, (long long)frac);
    }
    while (len > 0 && buf[len - 1] == '0') len--;
    if (len > 0 && buf[len - 1] == '.') len--;
    buf[len] = '\0';
    return buf;
}

// v1.4.5 W4: fmt"{x:.N}" — fixed-precision double formatting. Negative N
// (e.g. {x:.0}) means at least one fractional digit ("%.0f" would print no
// point at all, which reads like an integer). Shares the TLS scratch
static DATARA_TLS char tls_float_prec_bufs[64][64];
static DATARA_TLS uint32_t tls_float_prec_idx = 0;

const char* datara_rt_float_to_str_prec(double v, int64_t prec) {
    if (isnan(v)) {
        return signbit(v) ? "-NaN" : "NaN";
    }
    if (isinf(v)) {
        return v < 0 ? "-Infinity" : "Infinity";
    }
    char* buf = tls_float_prec_bufs[(tls_float_prec_idx++) & 63];
    if (prec < 0) prec = 0;
    if (prec > 60) prec = 60;
    snprintf(buf, 64, "%.*f", (int)prec, v);
    return buf;
}

// v1.4.5 W4: fmt"{x:x}" / "{x:X}" / "{x:o}" / "{x:b}" — integer radix
// formatting. Signed values print their two's-complement bit pattern as an
// unsigned number (hex on negatives must stay predictable, not sign-prefixed).
// Uppercase requests uppercase hex digits.
const char* datara_rt_int_to_str_radix(int64_t v, int64_t radix, int64_t upper) {
    char* buf = datara_str_scratch_alloc(72);
    if (!buf) return "";
    const char* digits = upper ? "0123456789ABCDEF" : "0123456789abcdef";
    uint64_t uv = (uint64_t)v;
    char tmp[72];
    int pos = 0;
    uint64_t base = radix <= 1 ? 16 : (radix > 16 ? 16 : (uint64_t)radix);
    do {
        tmp[pos++] = digits[uv % base];
        uv /= base;
    } while (uv > 0);
    int len = 0;
    while (pos > 0) {
        buf[len++] = tmp[--pos];
    }
    buf[len] = '\0';
    return buf;
}

// v1.4.5 W4: fmt"{d:.N}" — fixed-precision Dec64 formatting. The mantissa is
// a fixed-point scale-10^4 value; precision N means N fractional digits
// (round-half-away-from-zero beyond the 4 stored digits), and N > 4 pads
// with zeros so {d:.6} prints exactly 6 decimals.
const char* datara_rt_dec64_to_str_prec(int64_t val, int64_t prec) {
    char* buf = datara_str_scratch_alloc(64);
    if (!buf) return "";
    if (prec < 0) prec = 0;
    if (prec > 40) prec = 40;
    int64_t scale = 1;
    for (int64_t i = 0; i < prec; i++) scale *= 10;
    int64_t scaled = val * scale; // units of 10^-prec
    // Round from the stored 10^-4 grid to the requested 10^-prec grid.
    if (prec < 4) {
        int64_t drop = scaled % 10000;
        scaled /= 10000;
        if (drop <= -5000) scaled--;
        else if (drop >= 5000) scaled++;
    }
    int64_t integer = scaled / scale;
    int64_t frac = scaled % scale;
    if (frac < 0) frac = -frac;
    if (scaled < 0 && integer == 0) {
        snprintf(buf, 64, "-0.%0*lld", (int)prec, (long long)frac);
    } else if (prec == 0) {
        snprintf(buf, 64, "%lld", (long long)integer);
    } else {
        snprintf(buf, 64, "%lld.%0*lld", (long long)integer, (int)prec, (long long)frac);
    }
    return buf;
}

void datara_rt_print_bool(int64_t v) {
    if (v) {
        datara_rt_buf_write("true", 4);
    } else {
        datara_rt_buf_write("false", 5);
    }
}

void datara_rt_print_space(void) {
    datara_rt_buf_write(" ", 1);
}

void datara_rt_print_newline(void) {
    datara_rt_buf_write("\n", 1);
    datara_rt_flush_if_tty();
}

void datara_rt_print_list(void* list) {
    if (!list) {
        datara_rt_buf_write("[]", 2);
        return;
    }
    int64_t* arr = (int64_t*)list;
    int64_t len = arr[0];
    datara_rt_buf_write("[", 1);
    for (int64_t i = 0; i < len; i++) {
        if (i > 0) {
            datara_rt_buf_write(", ", 2);
        }
        datara_rt_print_int(arr[i + 1]);
    }
    datara_rt_buf_write("]", 1);
}

void datara_rt_err_print_str(const char* s) {
    if (!s) {
        datara_rt_panic("invariant violation: null string pointer passed to 'err' statement");
    }
    datara_rt_err_buf_write(s, strlen(s));
}

void datara_rt_err_print_int(int64_t v) {
    char buf[32];
    size_t len = datara_fast_i64toa(v, buf);
    datara_rt_err_buf_write(buf, len);
}

// v1.4.5 W3: Dec64 value on stderr. There is no err-dec64 primitive, so the
// exact fixed-point text is produced by datara_rt_dec_to_str and written to
// the stderr buffer.
void datara_rt_err_print_dec64_str(int64_t v) {
    const char* s = datara_rt_dec_to_str(v);
    datara_rt_err_buf_write(s, strlen(s));
}

void datara_rt_err_print_float(double v) {
    char buf[64];
    if (isnan(v)) {
        const char* nan_str = signbit(v) ? "-NaN" : "NaN";
        datara_rt_err_buf_write(nan_str, strlen(nan_str));
        return;
    }
    if (isinf(v)) {
        const char* inf_str = v < 0 ? "-Infinity" : "Infinity";
        datara_rt_err_buf_write(inf_str, strlen(inf_str));
        return;
    }
    if (v == floor(v) && fabs(v) < 1e15) {
        int len = snprintf(buf, sizeof(buf), "%.0f", v);
        datara_rt_err_buf_write(buf, len);
    } else {
        int len = snprintf(buf, sizeof(buf), "%.17g", v);
        datara_rt_err_buf_write(buf, len);
    }
}

void datara_rt_err_print_bool(int64_t v) {
    if (v) {
        datara_rt_err_buf_write("true", 4);
    } else {
        datara_rt_err_buf_write("false", 5);
    }
}

void datara_rt_err_print_newline(void) {
    datara_rt_err_buf_write("\n", 1);
    if (!g_capture_enabled) {
        fflush(stderr);
    }
}

// Prelude built-in functions
void datara_rt_print(const char* s) {
    datara_rt_print_str(s);
    datara_rt_flush();
}

void datara_rt_eprintln(const char* s) {
    datara_rt_flush();
    fputs(s ? s : "", stderr);
    fputc('\n', stderr);
    fflush(stderr);
}

void datara_rt_print_backtrace(void) {
    fprintf(stderr, "stack backtrace:\n");
#ifdef _WIN32
    void* stack[64];
    USHORT frames = CaptureStackBackTrace(1, 64, stack, NULL);
    if (frames == 0) {
        fprintf(stderr, "  (empty backtrace)\n");
        fflush(stderr);
        return;
    }

    HANDLE hProcess = GetCurrentProcess();
    HMODULE hDbgHelp = LoadLibraryA("dbghelp.dll");
    typedef BOOL (WINAPI *SymInitializeFn)(HANDLE, PCSTR, BOOL);
    typedef BOOL (WINAPI *SymFromAddrFn)(HANDLE, DWORD64, PDWORD64, void*);
    typedef BOOL (WINAPI *SymGetLineFromAddr64Fn)(HANDLE, DWORD64, PDWORD, void*);
    typedef BOOL (WINAPI *SymCleanupFn)(HANDLE);

    SymInitializeFn pSymInit = NULL;
    SymFromAddrFn pSymFromAddr = NULL;
    SymGetLineFromAddr64Fn pSymGetLine = NULL;
    SymCleanupFn pSymCleanup = NULL;

    if (hDbgHelp) {
        pSymInit = (SymInitializeFn)GetProcAddress(hDbgHelp, "SymInitialize");
        pSymFromAddr = (SymFromAddrFn)GetProcAddress(hDbgHelp, "SymFromAddr");
        pSymGetLine = (SymGetLineFromAddr64Fn)GetProcAddress(hDbgHelp, "SymGetLineFromAddr64");
        pSymCleanup = (SymCleanupFn)GetProcAddress(hDbgHelp, "SymCleanup");
        if (pSymInit) {
            pSymInit(hProcess, NULL, TRUE);
        }
    }

    char buffer[sizeof(SYMBOL_INFO) + 256 * sizeof(char)];
    PSYMBOL_INFO symbol = (PSYMBOL_INFO)buffer;
    symbol->SizeOfStruct = sizeof(SYMBOL_INFO);
    symbol->MaxNameLen = 255;

    IMAGEHLP_LINE64 line_info;
    memset(&line_info, 0, sizeof(IMAGEHLP_LINE64));
    line_info.SizeOfStruct = sizeof(IMAGEHLP_LINE64);

    for (USHORT i = 0; i < frames; i++) {
        DWORD64 address = (DWORD64)(uintptr_t)stack[i];
        const char* name = "???";
        DWORD displacement = 0;
        char loc[512] = {0};

        if (pSymFromAddr && pSymFromAddr(hProcess, address, 0, symbol)) {
            name = symbol->Name;
        }

        if (pSymGetLine && pSymGetLine(hProcess, address, &displacement, &line_info)) {
            snprintf(loc, sizeof(loc), "      at %s:%lu", line_info.FileName, (unsigned long)line_info.LineNumber);
        }

        if (loc[0] != '\0') {
            fprintf(stderr, "  %2u: %s\n%s (0x%llx)\n", (unsigned int)i, name, loc, (unsigned long long)address);
        } else {
            fprintf(stderr, "  %2u: %s (0x%llx)\n", (unsigned int)i, name, (unsigned long long)address);
        }
    }

    if (hDbgHelp) {
        if (pSymCleanup) pSymCleanup(hProcess);
        FreeLibrary(hDbgHelp);
    }
#else
#if defined(DATARA_HAS_EXECINFO)
    void* stack[64];
    int frames = backtrace(stack, 64);
    char** symbols = backtrace_symbols(stack, frames);
    if (symbols) {
        for (int i = 1; i < frames; i++) {
            fprintf(stderr, "  %2d: %s\n", i - 1, symbols[i]);
        }
        free(symbols);
    } else {
        for (int i = 1; i < frames; i++) {
            fprintf(stderr, "  %2d: 0x%lx\n", i - 1, (unsigned long)(uintptr_t)stack[i]);
        }
    }
#else
    fprintf(stderr, "  (native backtrace not supported on this platform)\n");
#endif
#endif
    fflush(stderr);
}

void datara_rt_panic(const char* s) {
    datara_rt_flush();
    fprintf(stderr, "panic: %s\n", s ? s : "explicit panic");
    datara_rt_print_backtrace();
    fflush(stderr);
    exit(1);
}

void datara_rt_assert(int64_t cond, const char* msg) {
    if (!cond) {
        datara_rt_panic(msg ? msg : "assertion failed");
    }
}

void datara_rt_overflow_panic(void) {
    datara_rt_panic("integer overflow");
}

void datara_rt_div_zero_panic(void) {
    datara_rt_panic("integer division by zero");
}

int64_t datara_rt_checked_add(int64_t a, int64_t b) {
    int64_t res;
#if defined(__GNUC__) || defined(__clang__)
    if (__builtin_add_overflow(a, b, &res)) {
        datara_rt_panic("integer overflow");
    }
#else
    if ((b > 0 && a > INT64_MAX - b) || (b < 0 && a < INT64_MIN - b)) {
        datara_rt_panic("integer overflow");
    }
    res = a + b;
#endif
    return res;
}

int64_t datara_rt_checked_sub(int64_t a, int64_t b) {
    int64_t res;
#if defined(__GNUC__) || defined(__clang__)
    if (__builtin_sub_overflow(a, b, &res)) {
        datara_rt_panic("integer overflow");
    }
#else
    if ((b < 0 && a > INT64_MAX + b) || (b > 0 && a < INT64_MIN + b)) {
        datara_rt_panic("integer overflow");
    }
    res = a - b;
#endif
    return res;
}

int64_t datara_rt_checked_mul(int64_t a, int64_t b) {
    int64_t res;
#if defined(__GNUC__) || defined(__clang__)
    if (__builtin_mul_overflow(a, b, &res)) {
        datara_rt_panic("integer overflow");
    }
#else
    if (a > 0) {
        if (b > 0) {
            if (a > (INT64_MAX / b)) datara_rt_panic("integer overflow");
        } else {
            if (b < (INT64_MIN / a)) datara_rt_panic("integer overflow");
        }
    } else {
        if (b > 0) {
            if (a < (INT64_MIN / b)) datara_rt_panic("integer overflow");
        } else {
            if (a != 0 && b < (INT64_MAX / a)) datara_rt_panic("integer overflow");
        }
    }
    res = a * b;
#endif
    return res;
}

int64_t datara_rt_checked_div(int64_t a, int64_t b) {
    if (b == 0) {
        datara_rt_panic("integer division by zero");
    }
    if (a == INT64_MIN && b == -1) {
        datara_rt_panic("integer overflow");
    }
    return a / b;
}

int64_t datara_rt_checked_rem(int64_t a, int64_t b) {
    if (b == 0) {
        datara_rt_panic("integer division by zero");
    }
    if (a == INT64_MIN && b == -1) {
        datara_rt_panic("integer overflow");
    }
    return a % b;
}

int64_t datara_rt_saturating_add(int64_t a, int64_t b) {
    int64_t res;
#if defined(__GNUC__) || defined(__clang__)
    if (__builtin_add_overflow(a, b, &res)) {
        return (a >= 0) ? INT64_MAX : INT64_MIN;
    }
    return res;
#else
    if (b > 0 && a > INT64_MAX - b) return INT64_MAX;
    if (b < 0 && a < INT64_MIN - b) return INT64_MIN;
    return a + b;
#endif
}

int64_t datara_rt_saturating_sub(int64_t a, int64_t b) {
    int64_t res;
#if defined(__GNUC__) || defined(__clang__)
    if (__builtin_sub_overflow(a, b, &res)) {
        return (a >= 0) ? INT64_MAX : INT64_MIN;
    }
    return res;
#else
    if (b < 0 && a > INT64_MAX + b) return INT64_MAX;
    if (b > 0 && a < INT64_MIN + b) return INT64_MIN;
    return a - b;
#endif
}

int64_t datara_rt_saturating_mul(int64_t a, int64_t b) {
    int64_t res;
#if defined(__GNUC__) || defined(__clang__)
    if (__builtin_mul_overflow(a, b, &res)) {
        return ((a ^ b) >= 0) ? INT64_MAX : INT64_MIN;
    }
    return res;
#else
    if (a > 0) {
        if (b > 0 && a > (INT64_MAX / b)) return INT64_MAX;
        if (b < 0 && b < (INT64_MIN / a)) return INT64_MIN;
    } else if (a < 0) {
        if (b > 0 && a < (INT64_MIN / b)) return INT64_MIN;
        if (b < 0 && b < (INT64_MAX / a)) return INT64_MAX;
    }
    return a * b;
#endif
}

int64_t datara_rt_wrapping_add(int64_t a, int64_t b) {
    return (int64_t)((uint64_t)a + (uint64_t)b);
}

int64_t datara_rt_wrapping_sub(int64_t a, int64_t b) {
    return (int64_t)((uint64_t)a - (uint64_t)b);
}

int64_t datara_rt_wrapping_mul(int64_t a, int64_t b) {
    return (int64_t)((uint64_t)a * (uint64_t)b);
}

int64_t datara_rt_len(const char* s) {
    return s ? (int64_t)strlen(s) : 0;
}

const char* datara_rt_input(const char* prompt) {
    datara_rt_flush();
    if (prompt && prompt[0] != '\0') {
        datara_rt_print_str(prompt);
        datara_rt_flush();
    }
    size_t cap = 256;
    size_t len = 0;
    char* buf = (char*)malloc(cap);
    if (!buf) return "";

    int c;
    while ((c = getchar()) != EOF) {
        if (c == '\n') break;
        if (c == '\r') {
            int next_c = getchar();
            if (next_c != '\n' && next_c != EOF) {
                ungetc(next_c, stdin);
            }
            break;
        }
        if (len + 2 >= cap) {
            cap *= 2;
            char* new_buf = (char*)realloc(buf, cap);
            if (!new_buf) {
                buf[len] = '\0';
                break;
            }
            buf = new_buf;
        }
        buf[len++] = (char)c;
    }
    buf[len] = '\0';
    char* ret = datara_str_scratch_alloc(len + 1);
    if (ret) {
        memcpy(ret, buf, len + 1);
        free(buf);
        return ret;
    }
    return buf;
}

int64_t datara_rt_fast_read_int(void) {
    int c = getchar();
    while (c != EOF && c <= ' ') {
        c = getchar();
    }
    if (c == EOF) return 0;
    int sign = 1;
    if (c == '-') {
        sign = -1;
        c = getchar();
    } else if (c == '+') {
        c = getchar();
    }
    int64_t val = 0;
    while (c >= '0' && c <= '9') {
        val = val * 10 + (int64_t)(c - '0');
        c = getchar();
    }
    return val * sign;
}

double datara_rt_fast_read_float(void) {
    int c = getchar();
    while (c != EOF && c <= ' ') {
        c = getchar();
    }
    if (c == EOF) return 0.0;
    int sign = 1;
    if (c == '-') {
        sign = -1;
        c = getchar();
    } else if (c == '+') {
        c = getchar();
    }
    double val = 0.0;
    while (c >= '0' && c <= '9') {
        val = val * 10.0 + (double)(c - '0');
        c = getchar();
    }
    if (c == '.') {
        double factor = 0.1;
        c = getchar();
        while (c >= '0' && c <= '9') {
            val += (double)(c - '0') * factor;
            factor *= 0.1;
            c = getchar();
        }
    }
    return val * sign;
}

int64_t datara_rt_input_int(const char* prompt) {
    if (prompt && prompt[0] != '\0') {
        const char* s = datara_rt_input(prompt);
        if (!s || s[0] == '\0') return 0;
        return (int64_t)strtoll(s, NULL, 10);
    }
    return datara_rt_fast_read_int();
}

double datara_rt_input_float(const char* prompt) {
    if (prompt && prompt[0] != '\0') {
        const char* s = datara_rt_input(prompt);
        if (!s || s[0] == '\0') return 0.0;
        return strtod(s, NULL);
    }
    return datara_rt_fast_read_float();
}

int64_t datara_rt_str_eq(const char* a, const char* b) {
    if (a == b) return 1;
    if (!a || !b) return 0;
    return strcmp(a, b) == 0 ? 1 : 0;
}

int64_t datara_rt_str_cmp(const char* a, const char* b) {
    if (a == b) return 0;
    if (!a) return -1;
    if (!b) return 1;
    int res = strcmp(a, b);
    if (res < 0) return -1;
    if (res > 0) return 1;
    return 0;
}

const char* datara_rt_str_from_byte(int64_t b) {
    char* buf = (char*)malloc(2);
    if (!buf) return "";
    buf[0] = (char)(b & 0xFF);
    buf[1] = '\0';
    return buf;
}

const char* datara_rt_str_from_bytes(const int64_t* bytes) {
    if (!bytes || bytes[0] <= 0) return "";
    int64_t len = bytes[0];
    char* buf = (char*)malloc((size_t)len + 1);
    if (!buf) return "";
    for (int64_t i = 0; i < len; i++) {
        buf[i] = (char)(bytes[i + 1] & 0xFF);
    }
    buf[len] = '\0';
    return buf;
}

int64_t* datara_rt_str_bytes(const char* s) {
    if (!s) return datara_rt_list_create(0);
    size_t len = strlen(s);
    int64_t* list = datara_rt_list_create((int64_t)len);
    for (size_t i = 0; i < len; i++) {
        list = datara_rt_list_append(list, (int64_t)(unsigned char)s[i]);
    }
    return list;
}

int64_t datara_rt_str_len(const char* s) {
    return s ? (int64_t)strlen(s) : 0;
}

int64_t datara_rt_byte_len(const char* s) {
    return s ? (int64_t)strlen(s) : 0;
}

