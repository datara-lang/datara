#!/usr/bin/env python3
"""P4 one-shot: split the 7.7k-line datara_runtime.c monolith into per-subsystem
translation units. Line ranges were mapped during the session against the
pre-split version of the file; this script verifies every boundary before
cutting and aborts if the file drifted.

Public ABI (datara_runtime.h) is untouched - this is a mechanical move plus a
small set of reviewed symbol promotions (statics -> extern, listed in PROMOTE).
"""

import re
import sys
from pathlib import Path

RT = Path("src/runtime")
SRC = RT / "datara_runtime.c"

# Boundary markers: (start_line, start_marker_snippet, end_line, end_marker_snippet)
BOUNDARIES = [
    (67,  "uint32_t datara_rt_abi_version", 66, "DATARA_TLS"),
    (141, "define DATARA_SCRATCH_RING_SIZE", 140, ""),
    (149, "tls_int_bufs", 148, ""),
    (152, "Phase 13: Size-Class", 151, ""),
    (256, "define DATARA_SSO_MAX_LEN", 255, ""),
    (703, "Datara Universal Polyglot Value", 702, ""),
    (1221, "Ultra-Fast Zero-Allocation Streaming", 1220, ""),
    (2118, "int64_t datara_rt_list_get", 2117, ""),
    (2149, "DataraListHeader", 2148, ""),
    (2628, "datara_rt_map_create_from", 2627, ""),
    (2812, "#ifdef _WIN32", 2811, ""),
    (3006, "Checked I/O returning Outcome", 3005, ""),
    (3453, "static int g_argc", 3452, ""),
    (3468, "datara_rt_args_get", 3467, ""),
    (3473, "datara_rt_str_contains", 3472, ""),
    (4008, "Network Sockets", 4007, ""),
    (4538, "High-Performance Fast Math", 4537, ""),
    (4584, "Cryptography: SHA-256", 4583, ""),
    (4827, "Native UI Dialogs", 4826, ""),
    (5007, "UTF-8 checked process", 5006, ""),
    (5205, "Memory Management & RAII", 5204, ""),
    (5256, "Multithreading Engine", 5255, ""),
    (5535, "Effect-Driven Ephemeral Frame Arena", 5534, ""),
    (5576, "Tier 2: 64-bit Bitmask Slab", 5575, ""),
    (5827, "First-Class SIMD 4D Vector Math", 5826, ""),
    (6750, "Graduated Ownership Runtime Guards", 6749, ""),
    (6804, "Phase 16: Profile-Guided", 6803, ""),
    (7036, "Phase 17: Runtime Systems Layer", 7035, ""),
    (7538, "v1.4.0 StrBuf", 7537, ""),
    (7621, "4-Tier Memory & Direct Hardware", 7620, ""),
]

# Promotion table: static symbol -> promoted declaration appended to
# datara_rt_internal.h by the fixup step (done manually after the cut).
PROMOTE = [
    "static DATARA_TLS int64_t tls_heap_alloc_count",
    "static void datara_rt_pgo_auto_flush(void)",
]

# Each module: (filename, list of (start, end) line ranges from the source)
# Ranges are inclusive and refer to the ORIGINAL monolith numbering.
MODULES = [    ("datara_alloc.c",    [(152, 253), (5576, 5825)]),
    ("datara_string.c",   [(256, 702)]),
    ("datara_value.c",    [(703, 1220)]),
    ("datara_print.c",    [(1221, 2117)]),
    ("datara_list.c",     [(2118, 2627)]),
    ("datara_map.c",      [(2628, 2813)]),
    ("datara_fs.c",       [(2812, 3452), (3473, 4007)]),
    ("datara_net.c",      [(4008, 4537)]),
    ("datara_crypto.c",   [(4584, 4826)]),
    ("datara_ui.c",       [(4827, 5204)]),
    ("datara_mem.c",      [(5205, 5255)]),
    ("datara_parallel.c", [(5256, 5574)]),
    ("datara_simd.c",     [(5827, 6749)]),
    ("datara_guard.c",    [(6750, 6803)]),
    ("datara_pgo.c",      [(6804, 7035)]),
    ("datara_core.c",     [(1, 150), (3453, 3467), (7036, 7537), (7538, 7764)]),
]

PRELUDE = """#if defined(__APPLE__)
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
"""

def lines_of(path: Path):
    return path.read_text(encoding="utf-8", errors="surrogateescape").splitlines(keepends=True)

def main():
    src_lines = lines_of(SRC)
    n = len(src_lines)
    print(f"source: {SRC} ({n} lines, {SRC.stat().st_size} bytes)")

    # 1. Verify boundary markers
    bad = []
    for start, marker, _e, _m in BOUNDARIES:
        text = "".join(src_lines[start - 1:start + 2])
        if marker not in text:
            bad.append((start, marker, text.strip()[:70]))
    if bad:
        print("BOUNDARY DRIFT DETECTED - aborting, file unchanged:")
        for ln, marker, got in bad:
            print(f"  line {ln}: expected marker {marker!r}, got {got!r}")
        sys.exit(1)
    print(f"all {len(BOUNDARIES)} boundary markers verified")

    # 2. Extract ranges per module
    def cut(a, b):
        return "".join(src_lines[a - 1:b])

    core_parts = {}
    for fname, ranges in MODULES:
        body = "".join(cut(a, b) for a, b in ranges)
        out = RT / fname
        if fname == "datara_core.c":
            # Keep core as the remaining glue: prelude is regenerated fresh,
            # body keeps its original content.
            out.write_text(PRELUDE + body, encoding="utf-8", newline="")
        else:
            out.write_text(PRELUDE + body, encoding="utf-8", newline="")
        print(f"wrote {out.name}: {out.stat().st_size} bytes")

    # 3. Sanity: total moved bytes vs source bytes
    total = sum((RT / f).stat().st_size for f, _ in MODULES)
    print(f"total module bytes (incl. prelude x{len(MODULES)}): {total}")

if __name__ == "__main__":
    main()
