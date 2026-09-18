#include "datara_memprofile.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
  #include <windows.h>
  #define ATOMIC_ADD64(ptr, val) InterlockedAdd64((LONG64 volatile*)(ptr), (LONG64)(val))
  #define ATOMIC_INC64(ptr) InterlockedIncrement64((LONG64 volatile*)(ptr))
#else
  #define ATOMIC_ADD64(ptr, val) __sync_add_and_fetch((ptr), (val))
  #define ATOMIC_INC64(ptr) __sync_add_and_fetch((ptr), 1)
#endif

static volatile int32_t g_memprof_active = 0;
static DataraMemSnapshot g_snapshot = {0};
static char g_json_buffer[2048] = {0};
static char g_out_file[512] = {0};

static void update_peak(void) {
    uint64_t cur = g_snapshot.current_live_bytes;
    while (cur > g_snapshot.peak_live_bytes) {
        g_snapshot.peak_live_bytes = cur;
    }
}

static void datara_memprofile_atexit(void) {
    if (g_out_file[0] != '\0') {
        datara_memprofile_dump(g_out_file);
    }
}

void datara_memprofile_start(void) {
    g_memprof_active = 1;
}

void datara_memprofile_stop(void) {
    g_memprof_active = 0;
}

bool datara_memprofile_is_active(void) {
    return g_memprof_active != 0;
}

void datara_memprofile_reset(void) {
    memset(&g_snapshot, 0, sizeof(g_snapshot));
}

void datara_memprofile_record_alloc(size_t bytes, int32_t tag) {
    if (!g_memprof_active && !getenv("DATARA_MEMPROFILE") && !getenv("DATARA_MEMPROFILE_OUT")) return;

    ATOMIC_INC64(&g_snapshot.total_alloc_count);
    ATOMIC_ADD64(&g_snapshot.total_allocated_bytes, bytes);
    ATOMIC_ADD64(&g_snapshot.current_live_bytes, bytes);
    update_peak();

    if (tag == DATARA_MEM_TAG_ARENA) {
        ATOMIC_ADD64(&g_snapshot.arena_bytes, bytes);
    } else if (tag == DATARA_MEM_TAG_SCRATCHPAD) {
        ATOMIC_ADD64(&g_snapshot.scratchpad_bytes, bytes);
    }
}

void datara_memprofile_record_free(size_t bytes, int32_t tag) {
    if (!g_memprof_active && !getenv("DATARA_MEMPROFILE") && !getenv("DATARA_MEMPROFILE_OUT")) return;

    ATOMIC_INC64(&g_snapshot.total_free_count);
    ATOMIC_ADD64(&g_snapshot.total_freed_bytes, bytes);

    if (g_snapshot.current_live_bytes >= bytes) {
        ATOMIC_ADD64(&g_snapshot.current_live_bytes, -(int64_t)bytes);
    } else {
        g_snapshot.current_live_bytes = 0;
    }

    if (tag == DATARA_MEM_TAG_ARENA && g_snapshot.arena_bytes >= bytes) {
        ATOMIC_ADD64(&g_snapshot.arena_bytes, -(int64_t)bytes);
    } else if (tag == DATARA_MEM_TAG_SCRATCHPAD && g_snapshot.scratchpad_bytes >= bytes) {
        ATOMIC_ADD64(&g_snapshot.scratchpad_bytes, -(int64_t)bytes);
    }
}

void datara_memprofile_record_promote(size_t bytes) {
    if (!g_memprof_active && !getenv("DATARA_MEMPROFILE") && !getenv("DATARA_MEMPROFILE_OUT")) return;

    ATOMIC_INC64(&g_snapshot.promotions_count);
    ATOMIC_ADD64(&g_snapshot.promoted_bytes, bytes);
    ATOMIC_ADD64(&g_snapshot.arena_bytes, bytes);
}

void datara_memprofile_record_slab(bool hit) {
    if (!g_memprof_active && !getenv("DATARA_MEMPROFILE") && !getenv("DATARA_MEMPROFILE_OUT")) return;

    if (hit) {
        ATOMIC_INC64(&g_snapshot.slab_hits);
    } else {
        ATOMIC_INC64(&g_snapshot.slab_misses);
    }
}

void datara_memprofile_snapshot(DataraMemSnapshot* out_snapshot) {
    if (!out_snapshot) return;
    memcpy(out_snapshot, &g_snapshot, sizeof(DataraMemSnapshot));
}

const char* datara_memprofile_summary_json(void) {
    snprintf(g_json_buffer, sizeof(g_json_buffer),
        "{\n"
        "  \"total_alloc_count\": %llu,\n"
        "  \"total_free_count\": %llu,\n"
        "  \"total_allocated_bytes\": %llu,\n"
        "  \"total_freed_bytes\": %llu,\n"
        "  \"current_live_bytes\": %llu,\n"
        "  \"peak_live_bytes\": %llu,\n"
        "  \"arena_bytes\": %llu,\n"
        "  \"scratchpad_bytes\": %llu,\n"
        "  \"promotions_count\": %llu,\n"
        "  \"promoted_bytes\": %llu,\n"
        "  \"slab_hits\": %llu,\n"
        "  \"slab_misses\": %llu\n"
        "}",
        (unsigned long long)g_snapshot.total_alloc_count,
        (unsigned long long)g_snapshot.total_free_count,
        (unsigned long long)g_snapshot.total_allocated_bytes,
        (unsigned long long)g_snapshot.total_freed_bytes,
        (unsigned long long)g_snapshot.current_live_bytes,
        (unsigned long long)g_snapshot.peak_live_bytes,
        (unsigned long long)g_snapshot.arena_bytes,
        (unsigned long long)g_snapshot.scratchpad_bytes,
        (unsigned long long)g_snapshot.promotions_count,
        (unsigned long long)g_snapshot.promoted_bytes,
        (unsigned long long)g_snapshot.slab_hits,
        (unsigned long long)g_snapshot.slab_misses
    );
    return g_json_buffer;
}

int32_t datara_memprofile_dump(const char* filepath) {
    if (!filepath || filepath[0] == '\0') return -1;
    FILE* f = fopen(filepath, "w");
    if (!f) return -2;

    const char* json = datara_memprofile_summary_json();
    fputs(json, f);
    fclose(f);
    return 0;
}

#ifdef _MSC_VER
#pragma section(".CRT$XCU", read)
static void __cdecl datara_memprofile_init_env(void);
__declspec(allocate(".CRT$XCU")) static void (__cdecl *p_init_memprof)(void) = datara_memprofile_init_env;
static void __cdecl datara_memprofile_init_env(void) {
#else
__attribute__((constructor)) static void datara_memprofile_init_env(void) {
#endif
    const char* out = getenv("DATARA_MEMPROFILE_OUT");
    if (out && out[0] != '\0') {
        strncpy(g_out_file, out, sizeof(g_out_file) - 1);
        g_memprof_active = 1;
        atexit(datara_memprofile_atexit);
    } else if (getenv("DATARA_MEMPROFILE")) {
        g_memprof_active = 1;
    }
}
