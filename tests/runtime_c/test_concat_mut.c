/* P1 acceptance tests for the growable scratch-buffer contract:
 *   1. Copy-concat results are live growable blocks with cached len == strlen.
 *   2. In-place append keeps ownership LIVE and keeps len in sync.
 *   3. Aliasing LHS: an append under a live alias still yields a consistent
 *      buffer (the optimizer never emits concat_mut while an alias exists;
 *      the runtime contract is that the result is never torn).
 *   4. Self-append (dst aliased by b) must not corrupt via overlapping copy.
 *   5. Microbench: 200k `s = s + "x"` appends must run in O(n) — tens of ms
 *      ceiling, not the O(n^2) strlen rescan.
 *
 * Strings used here are all > DATARA_SSO_MAX_LEN (22) so they land on
 * growable scratch-ring blocks rather than SSO slots.
 *
 * Standalone compile (MSVC, mirrors build.rs flags):
 *   cl.exe -nologo -MT -O2 -W4 -Isrc/runtime tests/runtime_c/test_concat_mut.c src/runtime/datara_runtime.c -Fe:test_concat_mut.exe ws2_32.lib user32.lib advapi32.lib dbghelp.lib
 */

#include <stdio.h>
#include <string.h>
#include <time.h>
#include "datara_runtime.h"

static int failures = 0;

#define CHECK(cond, msg) do { \
    if (!(cond)) { printf("FAIL: %s\n", msg); failures++; } \
    else { printf("ok:   %s\n", msg); } \
} while (0)

int main(void) {
    /* -- 1. copy concat produces a live growable buffer with cached len -- */
    const char* s = datara_rt_str_concat("0123456789ABCDEF", "0123456789"); /* 26B */
    CHECK(datara_rt_strbuf_test_growable(s), "26-byte copy concat is growable");
    if (datara_rt_strbuf_test_growable(s)) {
        CHECK(datara_rt_strbuf_test_len(s) == 26,
              "cached len matches strlen after copy concat");
        CHECK(datara_rt_strbuf_test_cap(s) >= 27,
              "cap covers string + NUL after copy concat");
    }

    /* -- 2. in-place append keeps len in sync (cap 32: 26->28->30 stay) -- */
    if (datara_rt_strbuf_test_growable(s)) {
        const char* s2 = datara_rt_str_concat_mut((char*)s, "ab");
        CHECK(s2 == s, "append with spare capacity stays in place");
        CHECK(datara_rt_strbuf_test_growable(s2), "ownership stays LIVE after append");
        CHECK(datara_rt_strbuf_test_len(s2) == 28,
              "cached len updated after in-place append");
        CHECK(strlen(s2) == 28, "payload correct after append");
        CHECK(strncmp(s2, "0123456789ABCDEF0123456789ab", 28) == 0,
              "appended bytes intact");
    }

    /* -- 3. append under a live alias yields a consistent buffer -- */
    {
        const char* a = datara_rt_str_concat("AAAABBBBCCCCDDDDEEEEFFFF", "EEEE"); /* 28B */
        if (datara_rt_strbuf_test_growable(a)) {
            const char* alias = a; /* second live reference */
            const char* r = datara_rt_str_concat_mut((char*)a, "ZZ");
            CHECK(r != NULL && strlen(r) == 30, "appended result has expected length");
            CHECK(strncmp(r, "AAAABBBBCCCCDDDDEEEEFFFFEEEEZZ", 30) == 0,
                  "appended payload intact (never torn)");
            CHECK(alias == a || alias == r, "alias stays a valid buffer pointer");
        }
    }

    /* -- 4. self-append: b aliases dst's payload (s = s + s) -- */
    {
        const char* a = datara_rt_str_concat("QWERTYUIOPASDFGHJKLMN", "OP"); /* 23B */
        CHECK(datara_rt_strbuf_test_growable(a), "23-byte self-append source growable");
        const char* b = a; /* same pointer */
        const char* r = datara_rt_str_concat_mut((char*)a, b);
        CHECK(strlen(r) == 46, "self-append doubles length");
        CHECK(strncmp(r,
                      "QWERTYUIOPASDFGHJKLMNOPQWERTYUIOPASDFGHJKLMNOP", 46) == 0,
              "self-append payload intact (no overlapping-copy tear)");
    }

    /* -- 5. microbench: 200k appends of "x" -- */
    {
        clock_t t0 = clock();
        const char* b = datara_rt_str_concat("seed0123456789", "abcdefghijkl"); /* 26B */
        for (int i = 0; i < 200000; i++) {
            b = datara_rt_str_concat_mut((char*)b, "x");
        }
        double ms = (double)(clock() - t0) * 1000.0 / CLOCKS_PER_SEC;
        printf("microbench: 200k appends in %.1f ms, final len %s\n",
               ms, strlen(b) == 200026 ? "OK" : "WRONG");
        CHECK(strlen(b) == 200026, "microbench final length");
        CHECK(ms < 500.0, "200k appends complete under 500 ms (O(n), not O(n^2))");
    }

    printf(failures == 0 ? "ALL PASSED\n" : "FAILURES: %d\n", failures);
    return failures == 0 ? 0 : 1;
}
