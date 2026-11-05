/* bench_json: C reference (PLAN_V152 N3 honest artifact).
 *
 * Workload (identical semantics to bench_json/json_bench.dtr):
 *   Phase 1 VALIDATE: full RFC 8259 recursive-descent validation of the
 *           whole document (objects, arrays, strings with escapes,
 *           strict numbers, literals).
 *   Phase 2 EXTRACT: second scan over the top-level array; for every
 *           record object extract the "id" (Int) and "name" (Str)
 *           values; accumulate sum(id) and sum(len(name)).
 *
 * Build: gcc -O2 -o json_c json_c.c
 * Run:   ./json_c data_10mb.json
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#ifdef _WIN32
#include <windows.h>
static double now_ms(void) {
    static LARGE_INTEGER freq = {0};
    LARGE_INTEGER t;
    if (freq.QuadPart == 0) QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t);
    return t.QuadPart * 1000.0 / freq.QuadPart;
}
#else
static double now_ms(void) {
    struct timespec ts;
    timespec_get(&ts, TIME_UTC);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1e6;
}
#endif

static const char *g_src;
static size_t g_len;
static size_t g_pos;

static void skip_ws(void) {
    while (g_pos < g_len) {
        char c = g_src[g_pos];
        if (c == ' ' || c == '\t' || c == '\n' || c == '\r') g_pos++;
        else break;
    }
}

static int parse_value(int depth);

static int match_lit(const char *lit) {
    size_t n = strlen(lit);
    if (g_pos + n > g_len || memcmp(g_src + g_pos, lit, n) != 0) return 0;
    g_pos += n;
    return 1;
}

static int parse_string(void) {
    if (g_pos >= g_len || g_src[g_pos] != '"') return 0;
    g_pos++;
    while (g_pos < g_len) {
        unsigned char c = (unsigned char)g_src[g_pos];
        if (c == '"') { g_pos++; return 1; }
        if (c < 0x20) return 0;
        if (c == '\\') {
            g_pos++;
            if (g_pos >= g_len) return 0;
            char e = g_src[g_pos];
            if (e == 'u') {
                g_pos++;
                for (int k = 0; k < 4; k++) {
                    if (g_pos >= g_len) return 0;
                    char h = g_src[g_pos];
                    int ok = (h >= '0' && h <= '9') || (h >= 'a' && h <= 'f') || (h >= 'A' && h <= 'F');
                    if (!ok) return 0;
                    g_pos++;
                }
            } else if (e == '"' || e == '\\' || e == '/' || e == 'b' || e == 'f' || e == 'n' || e == 'r' || e == 't') {
                g_pos++;
            } else {
                return 0;
            }
        } else {
            g_pos++;
        }
    }
    return 0;
}

static int parse_number(void) {
    if (g_pos < g_len && g_src[g_pos] == '-') g_pos++;
    if (g_pos >= g_len || g_src[g_pos] < '0' || g_src[g_pos] > '9') return 0;
    if (g_src[g_pos] == '0') {
        g_pos++;
        if (g_pos < g_len && g_src[g_pos] >= '0' && g_src[g_pos] <= '9') return 0;
    } else {
        while (g_pos < g_len && g_src[g_pos] >= '0' && g_src[g_pos] <= '9') g_pos++;
    }
    if (g_pos < g_len && g_src[g_pos] == '.') {
        g_pos++;
        if (g_pos >= g_len || g_src[g_pos] < '0' || g_src[g_pos] > '9') return 0;
        while (g_pos < g_len && g_src[g_pos] >= '0' && g_src[g_pos] <= '9') g_pos++;
    }
    if (g_pos < g_len && (g_src[g_pos] == 'e' || g_src[g_pos] == 'E')) {
        g_pos++;
        if (g_pos < g_len && (g_src[g_pos] == '+' || g_src[g_pos] == '-')) g_pos++;
        if (g_pos >= g_len || g_src[g_pos] < '0' || g_src[g_pos] > '9') return 0;
        while (g_pos < g_len && g_src[g_pos] >= '0' && g_src[g_pos] <= '9') g_pos++;
    }
    return 1;
}

static int parse_object(int depth) {
    if (depth > 128) return 0;
    g_pos++; /* { */
    skip_ws();
    if (g_pos < g_len && g_src[g_pos] == '}') { g_pos++; return 1; }
    for (;;) {
        skip_ws();
        if (!parse_string()) return 0;
        skip_ws();
        if (g_pos >= g_len || g_src[g_pos] != ':') return 0;
        g_pos++;
        skip_ws();
        if (!parse_value(depth + 1)) return 0;
        skip_ws();
        if (g_pos < g_len && g_src[g_pos] == ',') { g_pos++; continue; }
        if (g_pos < g_len && g_src[g_pos] == '}') { g_pos++; return 1; }
        return 0;
    }
}

static int parse_array(int depth) {
    if (depth > 128) return 0;
    g_pos++; /* [ */
    skip_ws();
    if (g_pos < g_len && g_src[g_pos] == ']') { g_pos++; return 1; }
    for (;;) {
        skip_ws();
        if (!parse_value(depth + 1)) return 0;
        skip_ws();
        if (g_pos < g_len && g_src[g_pos] == ',') { g_pos++; continue; }
        if (g_pos < g_len && g_src[g_pos] == ']') { g_pos++; return 1; }
        return 0;
    }
}

static int parse_value(int depth) {
    if (g_pos >= g_len) return 0;
    char c = g_src[g_pos];
    if (c == '{') return parse_object(depth);
    if (c == '[') return parse_array(depth);
    if (c == '"') return parse_string();
    if (c == '-' || (c >= '0' && c <= '9')) return parse_number();
    if (c == 't') { if (match_lit("true")) return 1; return 0; }
    if (c == 'f') { if (match_lit("false")) return 1; return 0; }
    if (c == 'n') { if (match_lit("null")) return 1; return 0; }
    return 0;
}

/* --- Phase 2: extraction --- */

/* Extracts an Int value for `key` inside [start,end): returns 0 if absent. */
static long long extract_int(size_t start, size_t end, const char *key) {
    char pat[64];
    snprintf(pat, sizeof pat, "\"%s\":", key);
    size_t plen = strlen(pat);
    const char *p = g_src + start;
    const char *pe = g_src + end;
    while (p + plen <= pe) {
        if (memcmp(p, pat, plen) == 0) {
            const char *v = p + plen;
            while (v < pe && (*v == ' ' || *v == '\t')) v++;
            int neg = 0;
            if (v < pe && *v == '-') { neg = 1; v++; }
            long long acc = 0;
            while (v < pe && *v >= '0' && *v <= '9') { acc = acc * 10 + (*v - '0'); v++; }
            return neg ? -acc : acc;
        }
        p++;
    }
    return 0;
}

/* Extracts strlen of the Str value for `key` inside [start,end). */
static long long extract_str_len(size_t start, size_t end, const char *key) {
    char pat[64];
    snprintf(pat, sizeof pat, "\"%s\":\"", key);
    size_t plen = strlen(pat);
    const char *p = g_src + start;
    const char *pe = g_src + end;
    while (p + plen <= pe) {
        if (memcmp(p, pat, plen) == 0) {
            long long n = 0;
            const char *v = p + plen;
            while (v < pe && *v != '"') { v++; n++; }
            return n;
        }
        p++;
    }
    return 0;
}

int main(int argc, char **argv) {
    if (argc < 2) { fprintf(stderr, "usage: json_c <file.json>\n"); return 2; }
    FILE *f = fopen(argv[1], "rb");
    if (!f) { perror("fopen"); return 2; }
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);
    char *buf = malloc((size_t)sz + 1);
    if (!buf || fread(buf, 1, (size_t)sz, f) != (size_t)sz) { fprintf(stderr, "read fail\n"); return 2; }
    buf[sz] = 0;
    fclose(f);
    g_src = buf;
    g_len = (size_t)sz;
    g_pos = 0;

    double t0 = now_ms();
    skip_ws();
    if (!parse_value(0)) { fprintf(stderr, "VALIDATE_FAIL at %zu\n", g_pos); return 1; }
    skip_ws();
    if (g_pos != g_len) { fprintf(stderr, "TRAILING at %zu\n", g_pos); return 1; }
    double t1 = now_ms();

    /* Phase 2: walk top-level array, extract per record. */
    long long sum_ids = 0, sum_name_len = 0, records = 0;
    size_t i = 0;
    int depth = 0;
    size_t rec_start = 0;
    while (i < g_len) {
        char c = g_src[i];
        if (c == '{') {
            if (depth == 1) rec_start = i;
            depth++;
        } else if (c == '}') {
            depth--;
            if (depth == 1) {
                sum_ids += extract_int(rec_start, i + 1, "id");
                sum_name_len += extract_str_len(rec_start, i + 1, "name");
                records++;
            }
        } else if (c == '[') {
            depth++;
        } else if (c == ']') {
            depth--;
        } else if (c == '"') {
            /* skip string content so braces inside strings don't count */
            i++;
            while (i < g_len && g_src[i] != '"') {
                if (g_src[i] == '\\') i++;
                i++;
            }
        }
        i++;
    }
    double t2 = now_ms();

    printf("RECORDS %lld\n", records);
    printf("SUM_IDS %lld\n", sum_ids);
    printf("SUM_NAME_LEN %lld\n", sum_name_len);
    printf("VALIDATE_MS %.1f\n", t1 - t0);
    printf("EXTRACT_MS %.1f\n", t2 - t1);
    free(buf);
    return 0;
}
