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
// Network Sockets (TCP/UDP)
// ---------------------------------------------------------------------------

static int g_wsa_initialized = 0;
static void datara_rt_ensure_sockets(void) {
#ifdef _WIN32
    if (!g_wsa_initialized) {
        WSADATA wsa;
        WSAStartup(MAKEWORD(2, 2), &wsa);
        g_wsa_initialized = 1;
    }
#endif
}

int64_t datara_rt_socket_create(int64_t is_tcp) {
    datara_rt_ensure_sockets();
    int type = is_tcp ? SOCK_STREAM : SOCK_DGRAM;
#ifdef _WIN32
    SOCKET s = socket(AF_INET, type, 0);
    if (s == INVALID_SOCKET) return -1;
    return (int64_t)s;
#else
    int s = socket(AF_INET, type, 0);
    if (s < 0) return -1;
    return (int64_t)s;
#endif
}

int64_t datara_rt_socket_bind(int64_t sock, const char* host, int64_t port) {
    if (sock < 0) return -1;
    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_port = htons((uint16_t)port);
    if (!host || strlen(host) == 0 || strcmp(host, "0.0.0.0") == 0) {
        addr.sin_addr.s_addr = INADDR_ANY;
    } else {
        addr.sin_addr.s_addr = inet_addr(host);
    }
    // No SO_REUSEADDR on Windows: there it lets a second socket bind the
    // same addr:port while the first is still listening (double-bind
    // hijack). On Unix it only affects lingering TIME_WAIT sockets, which
    // is the standard server-restart use case, so it stays on there. A
    // future socket_bind_reuse() builtin can re-enable it explicitly.
#ifndef _WIN32
    int opt = 1;
    setsockopt((int)sock, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));
#endif
#ifdef _WIN32
    if (bind((SOCKET)sock, (struct sockaddr*)&addr, sizeof(addr)) == SOCKET_ERROR) {
        return -1;
    }
#else
    if (bind((int)sock, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        return -1;
    }
#endif
    return 0;
}

int64_t datara_rt_socket_listen(int64_t sock, int64_t backlog) {
    datara_rt_cap_require(DATARA_CAP_NET_SERVER, "net::listen");
    if (sock < 0) return -1;
    int b = backlog > 0 ? (int)backlog : 128;
#ifdef _WIN32
    if (listen((SOCKET)sock, b) == SOCKET_ERROR) return -1;
#else
    if (listen((int)sock, b) < 0) return -1;
#endif
    return 0;
}

int64_t datara_rt_socket_accept(int64_t sock) {
    if (sock < 0) return -1;
#ifdef _WIN32
    SOCKET client = accept((SOCKET)sock, NULL, NULL);
    if (client == INVALID_SOCKET) return -1;
    return (int64_t)client;
#else
    int client = accept((int)sock, NULL, NULL);
    if (client < 0) return -1;
    return (int64_t)client;
#endif
}

int64_t datara_rt_socket_connect(int64_t sock, const char* host, int64_t port) {
    datara_rt_cap_require(DATARA_CAP_NET_CLIENT, "net::connect");
    if (sock < 0 || !host) return -1;
    char port_str[16];
    snprintf(port_str, sizeof(port_str), "%u", (unsigned int)port);
    struct addrinfo hints, *res = NULL;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    if (getaddrinfo(host, port_str, &hints, &res) != 0 || !res) {
        return -1;
    }
    int connect_res = -1;
    for (struct addrinfo* p = res; p != NULL; p = p->ai_next) {
#ifdef _WIN32
        if (connect((SOCKET)sock, p->ai_addr, (int)p->ai_addrlen) != SOCKET_ERROR) {
            connect_res = 0;
            break;
        }
#else
        if (connect((int)sock, p->ai_addr, p->ai_addrlen) == 0) {
            connect_res = 0;
            break;
        }
#endif
    }
    freeaddrinfo(res);
    return connect_res;
}

int64_t datara_rt_socket_send(int64_t sock, const char* data) {
    if (sock < 0 || !data) return -1;
    int len = (int)strlen(data);
#ifdef _WIN32
    int sent = send((SOCKET)sock, data, len, 0);
    return sent == SOCKET_ERROR ? -1 : (int64_t)sent;
#else
    ssize_t sent = send((int)sock, data, len, 0);
    return sent < 0 ? -1 : (int64_t)sent;
#endif
}

const char* datara_rt_socket_recv(int64_t sock, int64_t max_bytes) {
    if (sock < 0) return "";
    int cap = max_bytes > 0 ? (int)max_bytes : 4096;
    char* buf = (char*)malloc(cap + 1);
    if (!buf) return "";
#ifdef _WIN32
    int n = recv((SOCKET)sock, buf, cap, 0);
#else
    ssize_t n = recv((int)sock, buf, cap, 0);
#endif
    if (n <= 0) {
        free(buf);
        return "";
    }
    char* scratch = datara_str_scratch_alloc((size_t)n);
    if (scratch) {
        memcpy(scratch, buf, n);
        scratch[n] = '\0';
        free(buf);
        return scratch;
    }
    buf[n] = '\0';
    return buf;
}

void datara_rt_socket_close(int64_t sock) {
    if (sock < 0) return;
#ifdef _WIN32
    closesocket((SOCKET)sock);
#else
    close((int)sock);
#endif
}

void* datara_rt_socket_set_timeout(int64_t sock, int64_t ms) {
    if (sock < 0) return datara_rt_outcome_build_raw(0, 0, "invalid socket handle");
#ifdef _WIN32
    DWORD timeout = (DWORD)(ms > 0 ? ms : 0);
    int rc1 = setsockopt((SOCKET)sock, SOL_SOCKET, SO_RCVTIMEO, (const char*)&timeout, sizeof(timeout));
    int rc2 = setsockopt((SOCKET)sock, SOL_SOCKET, SO_SNDTIMEO, (const char*)&timeout, sizeof(timeout));
    if (rc1 == SOCKET_ERROR || rc2 == SOCKET_ERROR) {
        return datara_rt_outcome_build_raw(0, 0, "setsockopt SO_RCVTIMEO failed");
    }
#else
    struct timeval tv;
    tv.tv_sec = (time_t)(ms / 1000);
    tv.tv_usec = (suseconds_t)((ms % 1000) * 1000);
    int rc1 = setsockopt((int)sock, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof(tv));
    int rc2 = setsockopt((int)sock, SOL_SOCKET, SO_SNDTIMEO, &tv, sizeof(tv));
    if (rc1 < 0 || rc2 < 0) {
        return datara_rt_outcome_build_raw(0, 0, "setsockopt SO_RCVTIMEO failed");
    }
#endif
    return datara_rt_outcome_build_raw(1, 0, "");
}

void* datara_rt_socket_nonblocking(int64_t sock, int64_t on) {
    if (sock < 0) return datara_rt_outcome_build_raw(0, 0, "invalid socket handle");
#ifdef _WIN32
    u_long mode = on ? 1 : 0;
    if (ioctlsocket((SOCKET)sock, FIONBIO, &mode) != 0) {
        return datara_rt_outcome_build_raw(0, 0, "ioctlsocket FIONBIO failed");
    }
#else
    int flags = fcntl((int)sock, F_GETFL, 0);
    if (flags < 0) return datara_rt_outcome_build_raw(0, 0, "fcntl F_GETFL failed");
    if (on) {
        flags |= O_NONBLOCK;
    } else {
        flags &= ~O_NONBLOCK;
    }
    if (fcntl((int)sock, F_SETFL, flags) < 0) {
        return datara_rt_outcome_build_raw(0, 0, "fcntl F_SETFL failed");
    }
#endif
    return datara_rt_outcome_build_raw(1, 0, "");
}

void* datara_rt_socket_recv_outcome(int64_t sock, int64_t max_bytes) {
    if (sock < 0) return datara_rt_outcome_build_raw(0, 0, "invalid socket handle");
    int cap = max_bytes > 0 ? (int)max_bytes : 4096;
    char* buf = (char*)malloc((size_t)cap + 1);
    if (!buf) return datara_rt_outcome_build_raw(0, 0, "out of memory");
#ifdef _WIN32
    int n = recv((SOCKET)sock, buf, cap, 0);
    if (n == SOCKET_ERROR) {
        int err = WSAGetLastError();
        free(buf);
        if (err == WSAEWOULDBLOCK || err == WSAETIMEDOUT) {
            return datara_rt_outcome_build(0, "", "would-block");
        }
        return datara_rt_outcome_build(0, "", "socket recv error");
    }
#else
    ssize_t n = recv((int)sock, buf, (size_t)cap, 0);
    if (n < 0) {
        int err = errno;
        free(buf);
        if (err == EWOULDBLOCK || err == EAGAIN || err == ETIMEDOUT) {
            return datara_rt_outcome_build(0, "", "would-block");
        }
        return datara_rt_outcome_build(0, "", "socket recv error");
    }
#endif
    if (n == 0) {
        free(buf);
        return datara_rt_outcome_build(0, "", "connection closed");
    }
    buf[n] = '\0';
    void* out = datara_rt_outcome_build(1, buf, "");
    free(buf);
    return out;
}

// ---------------------------------------------------------------------------
// HTTP (blocking HTTP/1.1 GET over the socket primitives above)
// ---------------------------------------------------------------------------

/* Upper bound on a downloaded body, mirroring the runtime's other
 * bounded-resource conventions (scratch ring, socket recv caps). */
#define DATARA_HTTP_MAX_BODY (16 * 1024 * 1024)

static const char* datara_http_error(const char* msg) {
    size_t prefix_len = strlen("HTTP_ERROR: ");
    size_t msg_len = msg ? strlen(msg) : 0;
    char* buf = datara_str_scratch_alloc(prefix_len + msg_len);
    if (!buf) return "HTTP_ERROR: out of memory";
    memcpy(buf, "HTTP_ERROR: ", prefix_len);
    memcpy(buf + prefix_len, msg, msg_len);
    buf[prefix_len + msg_len] = '\0';
    return buf;
}

/* Case-insensitive comparison of n bytes. */
static int datara_http_ci_eq(const char* a, const char* b, size_t n) {
    for (size_t i = 0; i < n; i++) {
        char ca = a[i], cb = b[i];
        if (ca >= 'A' && ca <= 'Z') ca = (char)(ca + 32);
        if (cb >= 'A' && cb <= 'Z') cb = (char)(cb + 32);
        if (ca != cb) return 0;
    }
    return 1;
}

/* Case-insensitive search for `needle` inside the bounded region
 * [hay, hay+hay_len). Returns a pointer to the match or NULL. */
static const char* datara_http_ci_find(const char* hay, size_t hay_len,
                                       const char* needle) {
    size_t nlen = strlen(needle);
    if (nlen == 0 || hay_len < nlen) return NULL;
    for (size_t i = 0; i + nlen <= hay_len; i++) {
        if (datara_http_ci_eq(hay + i, needle, nlen)) return hay + i;
    }
    return NULL;
}

/* Find a header line by name (case-insensitive) inside the header block
 * [hdr, hdr+hdr_len). On success returns a pointer to the value (after the
 * colon, leading whitespace skipped) and sets *vlen to its length;
 * returns NULL when the header is absent. */
static const char* datara_http_header_value(const char* hdr, size_t hdr_len,
                                            const char* name, size_t* vlen) {
    size_t nlen = strlen(name);
    size_t i = 0;
    while (i < hdr_len) {
        size_t eol = i;
        while (eol < hdr_len && hdr[eol] != '\n') eol++;
        size_t line_len = eol - i;
        if (line_len > 0 && hdr[i + line_len - 1] == '\r') line_len--;
        if (line_len > nlen + 1 && hdr[i + nlen] == ':'
            && datara_http_ci_eq(hdr + i, name, nlen)) {
            size_t v = i + nlen + 1;
            while (v < i + line_len && (hdr[v] == ' ' || hdr[v] == '\t')) v++;
            *vlen = (i + line_len) - v;
            return hdr + v;
        }
        i = eol + 1;
    }
    return NULL;
}

/* Decode an HTTP/1.1 chunked body in place: reads hex chunk-size lines
 * (ignoring chunk extensions after ';'), copies each chunk's bytes down,
 * skips the trailing CRLF, and stops at the 0-size terminating chunk
 * (trailers are ignored). Returns the decoded length, or (size_t)-1 on
 * malformed input / when the total exceeds DATARA_HTTP_MAX_BODY. */
static size_t datara_http_decode_chunked(char* body, size_t body_len) {
    size_t rd = 0, wr = 0;
    for (;;) {
        size_t line_end = rd;
        while (line_end < body_len && body[line_end] != '\n') line_end++;
        if (line_end >= body_len) return (size_t)-1;
        size_t line_len = line_end - rd;
        if (line_len > 0 && body[line_end - 1] == '\r') line_len--;
        size_t size = 0;
        size_t k = 0;
        while (k < line_len) {
            char c = body[rd + k];
            int d;
            if (c >= '0' && c <= '9') d = c - '0';
            else if (c >= 'a' && c <= 'f') d = c - 'a' + 10;
            else if (c >= 'A' && c <= 'F') d = c - 'A' + 10;
            else break; /* chunk extensions (';...') or trailing junk */
            size = size * 16 + (size_t)d;
            if (size > DATARA_HTTP_MAX_BODY) return (size_t)-1;
            k++;
        }
        if (k == 0) return (size_t)-1; /* no hex digits at all */
        rd = line_end + 1;
        if (size == 0) break; /* final chunk; ignore trailers */
        if (size > body_len - rd) return (size_t)-1;
        if (wr + size > DATARA_HTTP_MAX_BODY) return (size_t)-1;
        memmove(body + wr, body + rd, size);
        wr += size;
        rd += size;
        /* Skip the CRLF (tolerate a bare LF) after the chunk data. */
        if (rd < body_len && body[rd] == '\r') rd++;
        if (rd < body_len && body[rd] == '\n') {
            rd++;
        } else if (size > 0 && rd < body_len && body[rd - 1] != '\n') {
            return (size_t)-1;
        }
    }
    return wr;
}

const char* datara_rt_http_get(const char* url) {
    datara_rt_cap_require(DATARA_CAP_NET_CLIENT, "http::get");
    if (!url || !url[0]) {
        return datara_http_error("empty url");
    }
    datara_rt_ensure_sockets();

    // Parse http://host[:port]/path — https is explicitly unsupported.
    if (strncmp(url, "http://", 7) != 0) {
        if (strncmp(url, "https://", 8) == 0) {
            return datara_http_error("https not supported");
        }
        return datara_http_error("invalid url (expected http://...)");
    }
    const char* host_start = url + 7;
    const char* path_start = strchr(host_start, '/');
    const char* path = path_start ? path_start : "/";
    size_t host_len = path_start ? (size_t)(path_start - host_start) : strlen(host_start);
    if (host_len == 0 || host_len >= 256) {
        return datara_http_error("invalid host");
    }
    char host[256];
    memcpy(host, host_start, host_len);
    host[host_len] = '\0';
    int port = 80;
    char* colon = strchr(host, ':');
    if (colon) {
        port = atoi(colon + 1);
        *colon = '\0';
        if (port <= 0 || port > 65535) {
            return datara_http_error("invalid port");
        }
    }
    if (!host[0]) {
        return datara_http_error("invalid host");
    }

    int64_t sock = datara_rt_socket_create(1);
    if (sock < 0) {
        return datara_http_error("socket creation failed");
    }
    if (datara_rt_socket_connect(sock, host, (int64_t)port) != 0) {
        datara_rt_socket_close(sock);
        return datara_http_error("connection failed");
    }

    char req[2048];
    int rl = snprintf(req, sizeof(req),
                      "GET %s HTTP/1.1\r\nHost: %s\r\nConnection: close\r\n\r\n",
                      path, host);
    if (rl <= 0 || (size_t)rl >= sizeof(req)) {
        datara_rt_socket_close(sock);
        return datara_http_error("request too long");
    }
    size_t total_sent = 0;
    while (total_sent < (size_t)rl) {
#ifdef _WIN32
        int n = send((SOCKET)sock, req + total_sent, (int)((size_t)rl - total_sent), 0);
        if (n == SOCKET_ERROR) {
#else
        ssize_t n = send((int)sock, req + total_sent, (size_t)rl - total_sent, 0);
        if (n <= 0) {
#endif
            datara_rt_socket_close(sock);
            return datara_http_error("send failed");
        }
        total_sent += (size_t)n;
    }

    // Read the full response until EOF.
    size_t cap = 64 * 1024;
    size_t len = 0;
    char* raw = (char*)malloc(cap);
    if (!raw) {
        datara_rt_socket_close(sock);
        return datara_http_error("out of memory");
    }
    for (;;) {
        if (len + 8192 + 1 > cap) {
            if (cap >= DATARA_HTTP_MAX_BODY) {
                free(raw);
                datara_rt_socket_close(sock);
                return datara_http_error("response too large");
            }
            cap *= 2;
            char* grown = (char*)realloc(raw, cap);
            if (!grown) {
                free(raw);
                datara_rt_socket_close(sock);
                return datara_http_error("out of memory");
            }
            raw = grown;
        }
#ifdef _WIN32
        int n = recv((SOCKET)sock, raw + len, 8192, 0);
#else
        ssize_t n = recv((int)sock, raw + len, 8192, 0);
#endif
        if (n == 0) break;
        if (n < 0) {
            free(raw);
            datara_rt_socket_close(sock);
            return datara_http_error("recv failed");
        }
        len += (size_t)n;
    }
    datara_rt_socket_close(sock);

    // Skip headers up to \r\n\r\n (tolerate bare \n\n for lenient servers).
    char* body = NULL;
    for (size_t i = 0; i + 3 < len; i++) {
        if (raw[i] == '\r' && raw[i + 1] == '\n' && raw[i + 2] == '\r' && raw[i + 3] == '\n') {
            body = raw + i + 4;
            break;
        }
    }
    if (!body) {
        for (size_t i = 0; i + 1 < len; i++) {
            if (raw[i] == '\n' && raw[i + 1] == '\n') {
                body = raw + i + 2;
                break;
            }
        }
    }
    if (!body) {
        free(raw);
        return datara_http_error("malformed response");
    }

    // Header block spans [raw, body); it includes the separator, which the
    // per-line header scanner tolerates.
    const char* hdr = raw;
    size_t hdr_len = (size_t)(body - raw);
    size_t body_len = len - hdr_len;

    size_t te_vlen = 0;
    const char* te = datara_http_header_value(hdr, hdr_len, "transfer-encoding", &te_vlen);
    if (te && datara_http_ci_find(te, te_vlen, "chunked")) {
        size_t decoded = datara_http_decode_chunked((char*)body, body_len);
        if (decoded == (size_t)-1) {
            free(raw);
            return datara_http_error("malformed chunked body");
        }
        body_len = decoded;
    } else {
        // Honor Content-Length when present: return exactly that many bytes
        // of the body. If the server sent fewer bytes than advertised
        // (early close), fall back to what actually arrived.
        size_t cl_vlen = 0;
        const char* cl = datara_http_header_value(hdr, hdr_len, "content-length", &cl_vlen);
        if (cl && cl_vlen > 0) {
            size_t declared = 0;
            size_t k = 0;
            for (; k < cl_vlen; k++) {
                char c = cl[k];
                if (c < '0' || c > '9') break;
                declared = declared * 10 + (size_t)(c - '0');
                if (declared > DATARA_HTTP_MAX_BODY) break;
            }
            if (k == cl_vlen && declared <= body_len && declared <= DATARA_HTTP_MAX_BODY) {
                body_len = declared;
            }
        }
    }

    char* out = datara_str_scratch_alloc(body_len);
    if (!out) {
        free(raw);
        return datara_http_error("out of memory");
    }
    memcpy(out, body, body_len);
    out[body_len] = '\0';
    free(raw);
    return out;
}

