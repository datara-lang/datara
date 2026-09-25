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
// Cryptography: SHA-256 & Base64
// ---------------------------------------------------------------------------

typedef struct {
    uint8_t data[64];
    uint32_t datalen;
    uint64_t bitlen;
    uint32_t state[8];
} DATARA_SHA256_CTX;

#define D_ROTR(a,b) (((a) >> (b)) | ((a) << (32-(b))))
#define D_SIG0(x) (D_ROTR(x,2) ^ D_ROTR(x,13) ^ D_ROTR(x,22))
#define D_SIG1(x) (D_ROTR(x,6) ^ D_ROTR(x,11) ^ D_ROTR(x,25))
#define D_sig0(x) (D_ROTR(x,7) ^ D_ROTR(x,18) ^ ((x) >> 3))
#define D_sig1(x) (D_ROTR(x,17) ^ D_ROTR(x,19) ^ ((x) >> 10))
#define D_CH(x,y,z) (((x) & (y)) ^ (~(x) & (z)))
#define D_MAJ(x,y,z) (((x) & (y)) ^ ((x) & (z)) ^ ((y) & (z)))

static const uint32_t K_SHA256[64] = {
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
};

static void datara_sha256_transform(DATARA_SHA256_CTX *ctx, const uint8_t data[]) {
    uint32_t a, b, c, d, e, f, g, h, i, j, t1, t2, m[64];
    for (i = 0, j = 0; i < 16; ++i, j += 4)
        m[i] = ((uint32_t)data[j] << 24) | ((uint32_t)data[j + 1] << 16) | ((uint32_t)data[j + 2] << 8) | ((uint32_t)data[j + 3]);
    for ( ; i < 64; ++i)
        m[i] = D_sig1(m[i - 2]) + m[i - 7] + D_sig0(m[i - 15]) + m[i - 16];
    a = ctx->state[0]; b = ctx->state[1]; c = ctx->state[2]; d = ctx->state[3];
    e = ctx->state[4]; f = ctx->state[5]; g = ctx->state[6]; h = ctx->state[7];
    for (i = 0; i < 64; ++i) {
        t1 = h + D_SIG1(e) + D_CH(e,f,g) + K_SHA256[i] + m[i];
        t2 = D_SIG0(a) + D_MAJ(a,b,c);
        h = g; g = f; f = e; e = d + t1;
        d = c; c = b; b = a; a = t1 + t2;
    }
    ctx->state[0] += a; ctx->state[1] += b; ctx->state[2] += c; ctx->state[3] += d;
    ctx->state[4] += e; ctx->state[5] += f; ctx->state[6] += g; ctx->state[7] += h;
}

static void datara_sha256_init(DATARA_SHA256_CTX *ctx) {
    ctx->datalen = 0;
    ctx->bitlen = 0;
    ctx->state[0] = 0x6a09e667; ctx->state[1] = 0xbb67ae85;
    ctx->state[2] = 0x3c6ef372; ctx->state[3] = 0xa54ff53a;
    ctx->state[4] = 0x510e527f; ctx->state[5] = 0x9b05688c;
    ctx->state[6] = 0x1f83d9ab; ctx->state[7] = 0x5be0cd19;
}

static void datara_sha256_update(DATARA_SHA256_CTX *ctx, const uint8_t data[], size_t len) {
    size_t i;
    for (i = 0; i < len; ++i) {
        ctx->data[ctx->datalen] = data[i];
        ctx->datalen++;
        if (ctx->datalen == 64) {
            datara_sha256_transform(ctx, ctx->data);
            ctx->bitlen += 512;
            ctx->datalen = 0;
        }
    }
}

static void datara_sha256_final(DATARA_SHA256_CTX *ctx, uint8_t hash[]) {
    uint32_t i = ctx->datalen;
    if (ctx->datalen < 56) {
        ctx->data[i++] = 0x80;
        while (i < 56) ctx->data[i++] = 0x00;
    } else {
        ctx->data[i++] = 0x80;
        while (i < 64) ctx->data[i++] = 0x00;
        datara_sha256_transform(ctx, ctx->data);
        memset(ctx->data, 0, 56);
    }
    ctx->bitlen += (uint64_t)ctx->datalen * 8;
    ctx->data[63] = (uint8_t)(ctx->bitlen);
    ctx->data[62] = (uint8_t)(ctx->bitlen >> 8);
    ctx->data[61] = (uint8_t)(ctx->bitlen >> 16);
    ctx->data[60] = (uint8_t)(ctx->bitlen >> 24);
    ctx->data[59] = (uint8_t)(ctx->bitlen >> 32);
    ctx->data[58] = (uint8_t)(ctx->bitlen >> 40);
    ctx->data[57] = (uint8_t)(ctx->bitlen >> 48);
    ctx->data[56] = (uint8_t)(ctx->bitlen >> 56);
    datara_sha256_transform(ctx, ctx->data);
    for (i = 0; i < 4; ++i) {
        hash[i]      = (uint8_t)((ctx->state[0] >> (24 - i * 8)) & 0x000000ff);
        hash[i + 4]  = (uint8_t)((ctx->state[1] >> (24 - i * 8)) & 0x000000ff);
        hash[i + 8]  = (uint8_t)((ctx->state[2] >> (24 - i * 8)) & 0x000000ff);
        hash[i + 12] = (uint8_t)((ctx->state[3] >> (24 - i * 8)) & 0x000000ff);
        hash[i + 16] = (uint8_t)((ctx->state[4] >> (24 - i * 8)) & 0x000000ff);
        hash[i + 20] = (uint8_t)((ctx->state[5] >> (24 - i * 8)) & 0x000000ff);
        hash[i + 24] = (uint8_t)((ctx->state[6] >> (24 - i * 8)) & 0x000000ff);
        hash[i + 28] = (uint8_t)((ctx->state[7] >> (24 - i * 8)) & 0x000000ff);
    }
}

const char* datara_rt_sha256(const char* input) {
    if (!input) return "";
    DATARA_SHA256_CTX ctx;
    datara_sha256_init(&ctx);
    datara_sha256_update(&ctx, (const uint8_t*)input, strlen(input));
    uint8_t hash[32];
    datara_sha256_final(&ctx, hash);

    char* hex = (char*)malloc(65);
    if (!hex) return "";
    for (int i = 0; i < 32; i++) {
        snprintf(hex + (i * 2), 3, "%02x", hash[i]);
    }
    hex[64] = '\0';
    return hex;
}

static const char B64_CHARS[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

const char* datara_rt_base64_encode(const char* input) {
    if (!input) return "";
    size_t in_len = strlen(input);
    size_t out_len = 4 * ((in_len + 2) / 3);
    char* encoded = (char*)malloc(out_len + 1);
    if (!encoded) return "";

    size_t i, j = 0;
    for (i = 0; i < in_len; i += 3) {
        uint32_t octet_a = (uint8_t)input[i];
        uint32_t octet_b = (i + 1 < in_len) ? (uint8_t)input[i + 1] : 0;
        uint32_t octet_c = (i + 2 < in_len) ? (uint8_t)input[i + 2] : 0;

        uint32_t triple = (octet_a << 16) + (octet_b << 8) + octet_c;

        encoded[j++] = B64_CHARS[(triple >> 18) & 0x3F];
        encoded[j++] = B64_CHARS[(triple >> 12) & 0x3F];
        encoded[j++] = (i + 1 < in_len) ? B64_CHARS[(triple >> 6) & 0x3F] : '=';
        encoded[j++] = (i + 2 < in_len) ? B64_CHARS[triple & 0x3F] : '=';
    }
    encoded[out_len] = '\0';
    return encoded;
}

const char* datara_rt_base64_decode(const char* input) {
    if (!input) return "";
    size_t in_len = strlen(input);
    if (in_len % 4 != 0) return "";

    size_t out_len = in_len / 4 * 3;
    if (in_len > 0 && input[in_len - 1] == '=') out_len--;
    if (in_len > 1 && input[in_len - 2] == '=') out_len--;

    char* decoded = (char*)malloc(out_len + 1);
    if (!decoded) return "";

    static int b64_rev[256];
    static int rev_init = 0;
    if (!rev_init) {
        memset(b64_rev, -1, sizeof(b64_rev));
        for (int k = 0; k < 64; k++) b64_rev[(uint8_t)B64_CHARS[k]] = k;
        rev_init = 1;
    }

    size_t i, j = 0;
    for (i = 0; i < in_len; i += 4) {
        int a = b64_rev[(uint8_t)input[i]];
        int b = b64_rev[(uint8_t)input[i + 1]];
        int c = input[i + 2] == '=' ? 0 : b64_rev[(uint8_t)input[i + 2]];
        int d = input[i + 3] == '=' ? 0 : b64_rev[(uint8_t)input[i + 3]];

        if (a < 0 || b < 0 || c < 0 || d < 0) { free(decoded); return ""; }

        uint32_t triple = (a << 18) | (b << 12) | (c << 6) | d;
        if (j < out_len) decoded[j++] = (triple >> 16) & 0xFF;
        if (j < out_len) decoded[j++] = (triple >> 8) & 0xFF;
        if (j < out_len) decoded[j++] = triple & 0xFF;
    }
    decoded[out_len] = '\0';
    return decoded;
}

int g_datara_rng_insecure = 0;

int64_t datara_rt_random_bytes(uint8_t* buf, int64_t len) {
    if (!buf || len <= 0) return 0;
#ifdef _WIN32
    typedef unsigned char (__stdcall *DataraRtlGenRandomFn)(void*, unsigned long);
    static DataraRtlGenRandomFn s_pfnRtlGenRandom = NULL;
    static int s_rng_init = 0;
    if (!s_rng_init) {
        HMODULE hAdvApi = LoadLibraryA("advapi32.dll");
        if (hAdvApi) {
            s_pfnRtlGenRandom = (DataraRtlGenRandomFn)GetProcAddress(hAdvApi, "SystemFunction036");
        }
        s_rng_init = 1;
    }
    if (s_pfnRtlGenRandom && s_pfnRtlGenRandom((void*)buf, (unsigned long)len)) {
        return len;
    }
#else
    int fd = open("/dev/urandom", O_RDONLY);
    if (fd >= 0) {
        ssize_t r = read(fd, buf, (size_t)len);
        close(fd);
        if (r > 0) return (int64_t)r;
    }
#endif
    // Insecure clock-seeded LCG fallback: mark it so callers can detect that
    // the bytes are not cryptographically random (datara_rt_rng_is_insecure).
    g_datara_rng_insecure = 1;
    // High-entropy fallback using splitmix64 / xorshift with clock
    int64_t seed = datara_rt_now_precise_ms();
    for (int64_t i = 0; i < len; i++) {
        seed = seed * 6364136223846793005ULL + 1442695040888963407ULL;
        buf[i] = (uint8_t)((seed >> 32) ^ (seed & 0xFF));
    }
    return len;
}

int datara_rt_rng_is_insecure(void) {
    return g_datara_rng_insecure;
}

const char* datara_rt_uuid_v4(void) {
    uint8_t b[16];
    datara_rt_random_bytes(b, 16);
    // RFC 4122 v4 variant & version bits
    b[6] = (b[6] & 0x0F) | 0x40; // Version 4
    b[8] = (b[8] & 0x3F) | 0x80; // Variant 1 (RFC 4122)

    char* str = (char*)malloc(37);
    if (!str) return "";
    snprintf(str, 37, "%02x%02x%02x%02x-%02x%02x-%02x%02x-%02x%02x-%02x%02x%02x%02x%02x%02x",
        b[0], b[1], b[2], b[3],
        b[4], b[5],
        b[6], b[7],
        b[8], b[9],
        b[10], b[11], b[12], b[13], b[14], b[15]
    );
    return str;
}

