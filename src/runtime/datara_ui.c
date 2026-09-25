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
// ---------------------------------------------------------------------------
// Native UI Dialogs (Cross-platform)
// ---------------------------------------------------------------------------

// Escape a string for use inside a double-quoted shell argument. Escapes
// backslash, double quote, dollar and backtick so untrusted message/title
// text cannot terminate the quoting, expand shell variables or inject
// commands into `system()` invocations built from osascript/zenity.
static void datara_shell_escape_double(const char* src, char* dst, size_t dst_size) {
    size_t j = 0;
    if (dst_size == 0) return;
    dst[0] = '\0';
    if (!src) return;
    for (size_t i = 0; src[i] != '\0' && j + 2 < dst_size; i++) {
        char c = src[i];
        if (c == '\\' || c == '"' || c == '$' || c == '`') {
            dst[j++] = '\\';
        }
        dst[j++] = c;
    }
    dst[j] = '\0';
}

int64_t datara_rt_dialog_info(const char* title, const char* msg) {
#ifdef _WIN32
    HMODULE hUser = LoadLibraryA("user32.dll");
    if (hUser) {
        typedef int (WINAPI *MsgBoxFn)(HWND, LPCSTR, LPCSTR, UINT);
        MsgBoxFn pfn = (MsgBoxFn)GetProcAddress(hUser, "MessageBoxA");
        if (pfn) {
            pfn(NULL, msg ? msg : "", title ? title : "Information", 0x00000000L | 0x00000040L /* MB_OK | MB_ICONINFORMATION */);
            return 1;
        }
    }
#elif defined(__APPLE__)
    if (msg && title) {
        char cmd[2048];
        char emsg[512], etitle[512];
        datara_shell_escape_double(msg, emsg, sizeof(emsg));
        datara_shell_escape_double(title, etitle, sizeof(etitle));
        snprintf(cmd, sizeof(cmd), "osascript -e 'display dialog \"%s\" with title \"%s\" buttons {\"OK\"} default button \"OK\"'", emsg, etitle);
        if (system(cmd) == 0) return 1;
    }
#else
    if (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY")) {
        char cmd[2048];
        char emsg[512], etitle[512];
        datara_shell_escape_double(msg ? msg : "", emsg, sizeof(emsg));
        datara_shell_escape_double(title ? title : "Info", etitle, sizeof(etitle));
        snprintf(cmd, sizeof(cmd), "zenity --info --title=\"%s\" --text=\"%s\" 2>/dev/null", etitle, emsg);
        if (system(cmd) == 0) return 1;
    }
#endif
    printf("[%s] %s\n", title ? title : "Info", msg ? msg : "");
    fflush(stdout);
    return 1;
}

int64_t datara_rt_dialog_alert(const char* title, const char* msg) {
#ifdef _WIN32
    HMODULE hUser = LoadLibraryA("user32.dll");
    if (hUser) {
        typedef int (WINAPI *MsgBoxFn)(HWND, LPCSTR, LPCSTR, UINT);
        MsgBoxFn pfn = (MsgBoxFn)GetProcAddress(hUser, "MessageBoxA");
        if (pfn) {
            pfn(NULL, msg ? msg : "", title ? title : "Alert", 0x00000000L | 0x00000030L /* MB_OK | MB_ICONWARNING */);
            return 1;
        }
    }
#elif defined(__APPLE__)
    if (msg && title) {
        char cmd[2048];
        char emsg[512], etitle[512];
        datara_shell_escape_double(msg, emsg, sizeof(emsg));
        datara_shell_escape_double(title, etitle, sizeof(etitle));
        snprintf(cmd, sizeof(cmd), "osascript -e 'display alert \"%s\" message \"%s\" as warning'", etitle, emsg);
        if (system(cmd) == 0) return 1;
    }
#else
    if (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY")) {
        char cmd[2048];
        char emsg[512], etitle[512];
        datara_shell_escape_double(msg ? msg : "", emsg, sizeof(emsg));
        datara_shell_escape_double(title ? title : "Warning", etitle, sizeof(etitle));
        snprintf(cmd, sizeof(cmd), "zenity --warning --title=\"%s\" --text=\"%s\" 2>/dev/null", etitle, emsg);
        if (system(cmd) == 0) return 1;
    }
#endif
    fprintf(stderr, "[%s] %s\n", title ? title : "Warning", msg ? msg : "");
    fflush(stderr);
    return 1;
}

int64_t datara_rt_dialog_confirm(const char* title, const char* msg) {
#ifdef _WIN32
    HMODULE hUser = LoadLibraryA("user32.dll");
    if (hUser) {
        typedef int (WINAPI *MsgBoxFn)(HWND, LPCSTR, LPCSTR, UINT);
        MsgBoxFn pfn = (MsgBoxFn)GetProcAddress(hUser, "MessageBoxA");
        if (pfn) {
            int res = pfn(NULL, msg ? msg : "", title ? title : "Confirm", 0x00000004L | 0x00000020L /* MB_YESNO | MB_ICONQUESTION */);
            return (res == 6 /* IDYES */) ? 1 : 0;
        }
    }
#elif defined(__APPLE__)
    if (msg && title) {
        char cmd[2048];
        char emsg[512], etitle[512];
        datara_shell_escape_double(msg, emsg, sizeof(emsg));
        datara_shell_escape_double(title, etitle, sizeof(etitle));
        snprintf(cmd, sizeof(cmd), "osascript -e 'display dialog \"%s\" with title \"%s\" buttons {\"Cancel\", \"OK\"} default button \"OK\"'", emsg, etitle);
        int res = system(cmd);
        return (res == 0) ? 1 : 0;
    }
#else
    if (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY")) {
        char cmd[2048];
        char emsg[512], etitle[512];
        datara_shell_escape_double(msg ? msg : "", emsg, sizeof(emsg));
        datara_shell_escape_double(title ? title : "Confirm", etitle, sizeof(etitle));
        snprintf(cmd, sizeof(cmd), "zenity --question --title=\"%s\" --text=\"%s\" 2>/dev/null", etitle, emsg);
        int res = system(cmd);
        return (res == 0) ? 1 : 0;
    }
#endif
    printf("[%s] %s (y/n): ", title ? title : "Confirm", msg ? msg : "");
    fflush(stdout);
    int c = getchar();
    return (c == 'y' || c == 'Y') ? 1 : 0;
}

int64_t datara_rt_system(const char* cmd) {
    if (!cmd) return -1;
    return (int64_t)system(cmd);
}

// byte-transparent: the child's output is captured as raw bytes and
// returned as-is in whatever encoding the console produced -- no
// transcoding happens on this path. Use datara_rt_exec_utf8 when the
// output must be normalized to UTF-8.
const char* datara_rt_exec(const char* cmd) {
    if (!cmd) return "";
#ifdef _WIN32
    FILE* pipe = _popen(cmd, "rt");
#else
    FILE* pipe = popen(cmd, "r");
#endif
    if (!pipe) return "";
    size_t cap = 4096;
    size_t len = 0;
    char* buf = (char*)malloc(cap);
    if (!buf) {
#ifdef _WIN32
        _pclose(pipe);
#else
        pclose(pipe);
#endif
        return "";
    }
    char temp[512];
    while (fgets(temp, sizeof(temp), pipe)) {
        size_t tlen = strlen(temp);
        if (len + tlen + 1 > cap) {
            cap *= 2;
            char* new_buf = (char*)realloc(buf, cap);
            if (!new_buf) break;
            buf = new_buf;
        }
        memcpy(buf + len, temp, tlen);
        len += tlen;
    }
    buf[len] = '\0';
#ifdef _WIN32
    _pclose(pipe);
#else
    pclose(pipe);
#endif
    return buf;
}

// ---------------------------------------------------------------------------
// UTF-8 checked process execution: Outcome<Str>.
//
// Unlike datara_rt_exec (byte-transparent passthrough), this entry point
// guarantees the returned text is UTF-8:
//   * Windows: the command line is converted to UTF-16 with
//     MultiByteToWideChar(CP_UTF8, ...) -- never ANSI -- and run through
//     CreateProcessW with an anonymous pipe. stdout and stderr are merged
//     into that single pipe (2>&1 semantics), so interleaving is preserved
//     but the streams are not separately addressable. The captured bytes
//     are decoded with the console output code page (OEM code page when no
//     console is attached) into UTF-16 and re-encoded as CP_UTF8.
//   * POSIX: popen("r") capture with passthrough -- the child's stdout is
//     already UTF-8 on POSIX, stderr goes to the parent's stderr.
// Every failure path (null command, spawn failure, empty capture buffer,
// conversion failure) returns a failed Outcome with a diagnostic message
// in error_msg. The object layout is the stdlib Outcome<T> one, built by
// datara_rt_outcome_build_raw.
// ---------------------------------------------------------------------------
#ifdef _WIN32
void* datara_rt_exec_utf8(const char* cmd) {
    datara_rt_cap_require(DATARA_CAP_SYS_EXEC, "sys::exec_utf8");
    if (!cmd) {
        return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: command is null");
    }

    int wlen = MultiByteToWideChar(CP_UTF8, 0, cmd, -1, NULL, 0);
    if (wlen <= 0) {
        return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: command is not valid UTF-8");
    }
    wchar_t* wcmd = (wchar_t*)malloc((size_t)wlen * sizeof(wchar_t));
    if (!wcmd) {
        return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: out of memory");
    }
    MultiByteToWideChar(CP_UTF8, 0, cmd, -1, wcmd, wlen);

    SECURITY_ATTRIBUTES sa;
    sa.nLength = sizeof(sa);
    sa.lpSecurityDescriptor = NULL;
    sa.bInheritHandle = TRUE;

    HANDLE read_end = NULL;
    HANDLE write_end = NULL;
    if (!CreatePipe(&read_end, &write_end, &sa, 0)) {
        free(wcmd);
        return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: cannot create pipe");
    }
    // The read end stays with the parent only; an inherited read end would
    // keep the pipe open after the child exits and hang the read loop.
    SetHandleInformation(read_end, HANDLE_FLAG_INHERIT, 0);

    STARTUPINFOW si;
    PROCESS_INFORMATION pi;
    ZeroMemory(&si, sizeof(si));
    ZeroMemory(&pi, sizeof(pi));
    si.cb = sizeof(si);
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdInput = NULL;
    // Merged capture: both output streams share the single pipe.
    si.hStdOutput = write_end;
    si.hStdError = write_end;

    BOOL ok = CreateProcessW(NULL, wcmd, NULL, NULL, TRUE, CREATE_NO_WINDOW, NULL, NULL, &si, &pi);
    free(wcmd);
    CloseHandle(write_end);
    if (!ok) {
        CloseHandle(read_end);
        return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: cannot start process");
    }

    size_t cap = 4096;
    size_t len = 0;
    char* buf = (char*)malloc(cap);
    if (!buf) {
        CloseHandle(read_end);
        CloseHandle(pi.hThread);
        CloseHandle(pi.hProcess);
        return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: out of memory");
    }
    for (;;) {
        if (len + 1024 + 1 > cap) {
            size_t new_cap = cap * 2;
            char* new_buf = (char*)realloc(buf, new_cap);
            if (!new_buf) {
                free(buf);
                CloseHandle(read_end);
                CloseHandle(pi.hThread);
                CloseHandle(pi.hProcess);
                return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: out of memory");
            }
            buf = new_buf;
            cap = new_cap;
        }
        DWORD got = 0;
        if (!ReadFile(read_end, buf + len, 1024, &got, NULL) || got == 0) break;
        len += (size_t)got;
    }
    buf[len] = '\0';
    CloseHandle(read_end);
    WaitForSingleObject(pi.hProcess, INFINITE);
    DWORD exit_code = 0;
    GetExitCodeProcess(pi.hProcess, &exit_code);
    CloseHandle(pi.hThread);
    CloseHandle(pi.hProcess);

    if (exit_code != 0) {
        free(buf);
        return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: process exited with non-zero status");
    }

    // First try strict UTF-8 decoding: if the process output is already valid UTF-8,
    // decode directly with CP_UTF8 without mangling.
    UINT cp = CP_UTF8;
    int wide_len = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, buf, (int)len, NULL, 0);
    if (wide_len <= 0) {
        cp = GetOEMCP();
        if (cp == 0 || cp == CP_UTF8) cp = GetACP();
        wide_len = MultiByteToWideChar(cp, 0, buf, (int)len, NULL, 0);
        if (wide_len <= 0) {
            free(buf);
            return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: cannot decode process output");
        }
    }
    wchar_t* wide = (wchar_t*)malloc(((size_t)wide_len + 1) * sizeof(wchar_t));
    if (!wide) {
        free(buf);
        return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: out of memory");
    }
    MultiByteToWideChar(cp, 0, buf, (int)len, wide, wide_len);
    wide[wide_len] = L'\0';
    free(buf);

    int utf8_len = WideCharToMultiByte(CP_UTF8, 0, wide, wide_len, NULL, 0, NULL, NULL);
    if (utf8_len < 0) {
        free(wide);
        return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: cannot encode process output as UTF-8");
    }
    char* utf8 = (char*)malloc((size_t)utf8_len + 1);
    if (!utf8) {
        free(wide);
        return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: out of memory");
    }
    if (utf8_len > 0) {
        WideCharToMultiByte(CP_UTF8, 0, wide, wide_len, utf8, utf8_len, NULL, NULL);
    }
    utf8[utf8_len] = '\0';
    free(wide);
    // The returned string is intentionally never freed (runtime strings
    // leak by design, see datara_rt_str_free).
    return datara_rt_outcome_build_raw(1, (int64_t)(uintptr_t)utf8, "");
}
#else
void* datara_rt_exec_utf8(const char* cmd) {
    datara_rt_cap_require(DATARA_CAP_SYS_EXEC, "sys::exec_utf8");
    if (!cmd) {
        return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: command is null");
    }
    // POSIX: child output is already UTF-8 bytes; capture and pass through.
    FILE* pipe = popen(cmd, "r");
    if (!pipe) {
        return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: cannot start process");
    }
    size_t cap = 4096;
    size_t len = 0;
    char* buf = (char*)malloc(cap);
    if (!buf) {
        pclose(pipe);
        return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: out of memory");
    }
    char temp[1024];
    for (;;) {
        size_t got = fread(temp, 1, sizeof(temp), pipe);
        if (got == 0) break;
        if (len + got + 1 > cap) {
            size_t new_cap = cap;
            while (len + got + 1 > new_cap) new_cap *= 2;
            char* new_buf = (char*)realloc(buf, new_cap);
            if (!new_buf) {
                free(buf);
                pclose(pipe);
                return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: out of memory");
            }
            buf = new_buf;
            cap = new_cap;
        }
        memcpy(buf + len, temp, got);
        len += got;
    }
    buf[len] = '\0';
    int status = pclose(pipe);
    if (status != 0) {
        free(buf);
        return datara_rt_outcome_build_raw(0, 0, "exec_utf8 failed: process exited with non-zero status");
    }
    return datara_rt_outcome_build_raw(1, (int64_t)(uintptr_t)buf, "");
}
#endif

