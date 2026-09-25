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
// ============================================================================
// Datara Universal Polyglot Value (DataraValue) & 64-bit NaN-Boxing
// ============================================================================

DataraValue datara_val_from_float(double f) {
    DataraValue v;
    memcpy(&v, &f, sizeof(v));
    if ((v & DATARA_QNAN_MASK) == DATARA_QNAN_PREFIX && (v & DATARA_TAG_MASK) != 0) {
        v = DATARA_QNAN_PREFIX; // canonical float NaN
    }
    return v;
}

DataraValue datara_val_from_int(int64_t val) {
    uint64_t payload = (uint64_t)val & DATARA_PAYLOAD_MASK;
    return DATARA_QNAN_PREFIX | DATARA_TAG_INT | payload;
}

DataraValue datara_val_from_bool(int32_t b) {
    return DATARA_QNAN_PREFIX | DATARA_TAG_BOOL | (b ? 1ULL : 0ULL);
}

DataraValue datara_val_from_str(const char* s) {
    return DATARA_QNAN_PREFIX | DATARA_TAG_STR | (((uint64_t)(uintptr_t)s) & DATARA_PAYLOAD_MASK);
}

DataraValue datara_val_from_rawptr(void* ptr) {
    return DATARA_QNAN_PREFIX | DATARA_TAG_RAWPTR | (((uint64_t)(uintptr_t)ptr) & DATARA_PAYLOAD_MASK);
}

DataraValue datara_val_from_handle(uint32_t handle_id) {
    return DATARA_QNAN_PREFIX | DATARA_TAG_HANDLE | (uint64_t)handle_id;
}

DataraValue datara_val_null(void) {
    return DATARA_QNAN_PREFIX | DATARA_TAG_NULL;
}

DataraValue datara_val_undefined(void) {
    return DATARA_QNAN_PREFIX | DATARA_TAG_UNDEFINED;
}

int32_t datara_val_is_float(DataraValue v) {
    return ((v & DATARA_QNAN_MASK) != DATARA_QNAN_PREFIX) || ((v & DATARA_TAG_MASK) == 0);
}

int32_t datara_val_is_int(DataraValue v) {
    return ((v & DATARA_QNAN_MASK) == DATARA_QNAN_PREFIX) && ((v & DATARA_TAG_MASK) == DATARA_TAG_INT);
}

int32_t datara_val_is_bool(DataraValue v) {
    return ((v & DATARA_QNAN_MASK) == DATARA_QNAN_PREFIX) && ((v & DATARA_TAG_MASK) == DATARA_TAG_BOOL);
}

int32_t datara_val_is_str(DataraValue v) {
    return ((v & DATARA_QNAN_MASK) == DATARA_QNAN_PREFIX) && ((v & DATARA_TAG_MASK) == DATARA_TAG_STR);
}

int32_t datara_val_is_rawptr(DataraValue v) {
    return ((v & DATARA_QNAN_MASK) == DATARA_QNAN_PREFIX) && ((v & DATARA_TAG_MASK) == DATARA_TAG_RAWPTR);
}

int32_t datara_val_is_handle(DataraValue v) {
    return ((v & DATARA_QNAN_MASK) == DATARA_QNAN_PREFIX) && ((v & DATARA_TAG_MASK) == DATARA_TAG_HANDLE);
}

int32_t datara_val_is_null(DataraValue v) {
    return ((v & DATARA_QNAN_MASK) == DATARA_QNAN_PREFIX) && ((v & DATARA_TAG_MASK) == DATARA_TAG_NULL);
}

int32_t datara_val_is_undefined(DataraValue v) {
    return ((v & DATARA_QNAN_MASK) == DATARA_QNAN_PREFIX) && ((v & DATARA_TAG_MASK) == DATARA_TAG_UNDEFINED);
}

double datara_val_to_float(DataraValue v) {
    double d;
    memcpy(&d, &v, sizeof(d));
    return d;
}

int64_t datara_val_to_int(DataraValue v) {
    uint64_t payload = v & DATARA_PAYLOAD_MASK;
    if (payload & 0x0000800000000000ULL) {
        payload |= 0xFFFF000000000000ULL;
    }
    return (int64_t)payload;
}

int32_t datara_val_to_bool(DataraValue v) {
    return (v & 1ULL) ? 1 : 0;
}

const char* datara_val_to_str(DataraValue v) {
    return (const char*)(uintptr_t)(v & DATARA_PAYLOAD_MASK);
}

void* datara_val_to_rawptr(DataraValue v) {
    return (void*)(uintptr_t)(v & DATARA_PAYLOAD_MASK);
}

uint32_t datara_val_to_handle(DataraValue v) {
    return (uint32_t)(v & 0xFFFFFFFFULL);
}

// Legacy Nanbox backward compatibility
uint64_t datara_rt_nanbox_int(int64_t val) {
    return datara_val_from_int(val);
}

int64_t datara_rt_nanunbox_int(uint64_t box) {
    return datara_val_to_int(box);
}

uint64_t datara_rt_nanbox_bool(int64_t b) {
    return datara_val_from_bool(b ? 1 : 0);
}

int64_t datara_rt_nanunbox_bool(uint64_t box) {
    return (int64_t)datara_val_to_bool(box);
}

uint64_t datara_rt_nanbox_str(const char* s) {
    return datara_val_from_str(s);
}

const char* datara_rt_nanunbox_str(uint64_t box) {
    return datara_val_to_str(box);
}

void datara_rt_out_val(uint64_t box) {
    if (datara_val_is_float(box)) {
        datara_rt_out_float(datara_val_to_float(box));
    } else if (datara_val_is_int(box)) {
        datara_rt_out_int(datara_val_to_int(box));
    } else if (datara_val_is_bool(box)) {
        datara_rt_out_bool(datara_val_to_bool(box));
    } else if (datara_val_is_str(box)) {
        datara_rt_out_str(datara_val_to_str(box));
    } else if (datara_val_is_null(box) || datara_val_is_undefined(box)) {
        printf("None\n");
    } else if (datara_val_is_handle(box)) {
        uint32_t hid = datara_val_to_handle(box);
        const char* tag = datara_rt_handle_tag(hid);
        printf("<foreign handle #%u [%s]>\n", hid, tag ? tag : "unknown");
    } else {
        datara_rt_out_int((int64_t)box);
    }
}

// ============================================================================
// Foreign Handle Table Implementation (Thread-Safe, Mutex-Guarded, Auto-Growing)
// ============================================================================
typedef struct {
    void* ptr;
    const char* type_tag;
    void (*destructor)(void*);
    uint32_t generation;
    int32_t in_use;
} DataraHandleEntry;

static DataraHandleEntry* g_handles = NULL;
static size_t g_handles_cap = 0;
static size_t g_handles_count = 0;

#if defined(_WIN32)
static SRWLOCK g_handle_lock = SRWLOCK_INIT;
#define HANDLE_LOCK_ACQUIRE() AcquireSRWLockExclusive(&g_handle_lock)
#define HANDLE_LOCK_RELEASE() ReleaseSRWLockExclusive(&g_handle_lock)
#define HANDLE_LOCK_READ_ACQUIRE() AcquireSRWLockShared(&g_handle_lock)
#define HANDLE_LOCK_READ_RELEASE() ReleaseSRWLockShared(&g_handle_lock)
#else
static pthread_mutex_t g_handle_lock = PTHREAD_MUTEX_INITIALIZER;
#define HANDLE_LOCK_ACQUIRE() pthread_mutex_lock(&g_handle_lock)
#define HANDLE_LOCK_RELEASE() pthread_mutex_unlock(&g_handle_lock)
#define HANDLE_LOCK_READ_ACQUIRE() pthread_mutex_lock(&g_handle_lock)
#define HANDLE_LOCK_READ_RELEASE() pthread_mutex_unlock(&g_handle_lock)
#endif

uint32_t datara_rt_handle_alloc(void* ptr, const char* type_tag, void (*destructor)(void*)) {
    HANDLE_LOCK_ACQUIRE();
    if (!g_handles || g_handles_cap == 0) {
        size_t initial_cap = 64;
        g_handles = (DataraHandleEntry*)calloc(initial_cap, sizeof(DataraHandleEntry));
        if (!g_handles) {
            HANDLE_LOCK_RELEASE();
            return 0;
        }
        g_handles_cap = initial_cap;
    }

    // Slot 0 is reserved as null/invalid handle
    size_t found_idx = 0;
    for (size_t i = 1; i < g_handles_cap; ++i) {
        if (!g_handles[i].in_use) {
            found_idx = i;
            break;
        }
    }

    if (found_idx == 0) {
        size_t new_cap = g_handles_cap * 2;
        DataraHandleEntry* new_table = (DataraHandleEntry*)realloc(g_handles, new_cap * sizeof(DataraHandleEntry));
        if (!new_table) {
            HANDLE_LOCK_RELEASE();
            return 0;
        }
        memset(new_table + g_handles_cap, 0, (new_cap - g_handles_cap) * sizeof(DataraHandleEntry));
        found_idx = g_handles_cap;
        g_handles = new_table;
        g_handles_cap = new_cap;
    }

    g_handles[found_idx].ptr = ptr;
    g_handles[found_idx].type_tag = type_tag ? type_tag : "foreign";
    g_handles[found_idx].destructor = destructor;
    g_handles[found_idx].generation++;
    g_handles[found_idx].in_use = 1;
    g_handles_count++;

    HANDLE_LOCK_RELEASE();
    return (uint32_t)found_idx;
}

void* datara_rt_handle_get(uint32_t handle_id) {
    if (handle_id == 0) return NULL;
    HANDLE_LOCK_READ_ACQUIRE();
    void* res = NULL;
    if (g_handles && handle_id < g_handles_cap && g_handles[handle_id].in_use) {
        res = g_handles[handle_id].ptr;
    }
    HANDLE_LOCK_READ_RELEASE();
    return res;
}

const char* datara_rt_handle_tag(uint32_t handle_id) {
    if (handle_id == 0) return NULL;
    HANDLE_LOCK_READ_ACQUIRE();
    const char* res = NULL;
    if (g_handles && handle_id < g_handles_cap && g_handles[handle_id].in_use) {
        res = g_handles[handle_id].type_tag;
    }
    HANDLE_LOCK_READ_RELEASE();
    return res;
}

int32_t datara_rt_handle_free(uint32_t handle_id) {
    if (handle_id == 0) return 0;
    void* ptr = NULL;
    void (*dtor)(void*) = NULL;

    HANDLE_LOCK_ACQUIRE();
    if (g_handles && handle_id < g_handles_cap && g_handles[handle_id].in_use) {
        ptr = g_handles[handle_id].ptr;
        dtor = g_handles[handle_id].destructor;
        g_handles[handle_id].in_use = 0;
        g_handles[handle_id].ptr = NULL;
        g_handles[handle_id].type_tag = NULL;
        g_handles[handle_id].destructor = NULL;
        if (g_handles_count > 0) g_handles_count--;
        HANDLE_LOCK_RELEASE();
        if (dtor && ptr) {
            dtor(ptr);
        }
        return 1;
    }
    HANDLE_LOCK_RELEASE();
    return 0;
}

void datara_rt_handle_shutdown(void) {
    HANDLE_LOCK_ACQUIRE();
    if (g_handles) {
        for (size_t i = 1; i < g_handles_cap; ++i) {
            if (g_handles[i].in_use) {
                void* ptr = g_handles[i].ptr;
                void (*dtor)(void*) = g_handles[i].destructor;
                g_handles[i].in_use = 0;
                g_handles[i].ptr = NULL;
                if (dtor && ptr) {
                    dtor(ptr);
                }
            }
        }
        free(g_handles);
        g_handles = NULL;
        g_handles_cap = 0;
        g_handles_count = 0;
    }
    HANDLE_LOCK_RELEASE();
}

size_t datara_rt_handle_count(void) {
    HANDLE_LOCK_READ_ACQUIRE();
    size_t cnt = g_handles_count;
    HANDLE_LOCK_READ_RELEASE();
    return cnt;
}

// ============================================================================
// DataraMemoryView Implementation
// ============================================================================
size_t datara_dtype_itemsize(DataraDType dtype) {
    switch (dtype) {
        case DATARA_DTYPE_INT8:
        case DATARA_DTYPE_UINT8:
        case DATARA_DTYPE_BOOL:
            return 1;
        case DATARA_DTYPE_INT16:
        case DATARA_DTYPE_UINT16:
            return 2;
        case DATARA_DTYPE_INT32:
        case DATARA_DTYPE_UINT32:
        case DATARA_DTYPE_FLOAT32:
            return 4;
        case DATARA_DTYPE_INT64:
        case DATARA_DTYPE_UINT64:
        case DATARA_DTYPE_FLOAT64:
            return 8;
        default:
            return 8;
    }
}

DataraMemoryView datara_memview_1d(void* data, int64_t length, DataraDType dtype) {
    DataraMemoryView mv;
    memset(&mv, 0, sizeof(mv));
    mv.data = data;
    mv.element_type = dtype;
    size_t sz = datara_dtype_itemsize(dtype);
    mv.ndim = 1;
    mv.shape[0] = length >= 0 ? length : 0;
    mv.strides[0] = (int64_t)sz;
    mv.total_bytes = (size_t)mv.shape[0] * sz;
    return mv;
}

DataraMemoryView datara_memview_2d(void* data, int64_t rows, int64_t cols, DataraDType dtype) {
    DataraMemoryView mv;
    memset(&mv, 0, sizeof(mv));
    mv.data = data;
    mv.element_type = dtype;
    size_t sz = datara_dtype_itemsize(dtype);
    mv.ndim = 2;
    mv.shape[0] = rows >= 0 ? rows : 0;
    mv.shape[1] = cols >= 0 ? cols : 0;
    mv.strides[1] = (int64_t)sz;
    mv.strides[0] = (int64_t)(mv.shape[1] * sz);
    mv.total_bytes = (size_t)(mv.shape[0] * mv.shape[1]) * sz;
    return mv;
}

DataraMemoryView datara_memview_from_list_f64(int64_t* list) {
    if (!list) {
        return datara_memview_1d(NULL, 0, DATARA_DTYPE_FLOAT64);
    }
    int64_t count = list[0];
    if (count < 0) count = 0;
    void* data_ptr = (void*)(&list[1]);
    return datara_memview_1d(data_ptr, count, DATARA_DTYPE_FLOAT64);
}

DataraMemoryView datara_memview_from_vector(void* vec_data, int64_t len, DataraDType dtype) {
    return datara_memview_1d(vec_data, len, dtype);
}

int64_t datara_memview_get_f64(const DataraMemoryView* view, int64_t idx, double* out) {
    if (!view || !view->data || !out || idx < 0 || idx >= view->shape[0]) return 0;
    if (view->element_type == DATARA_DTYPE_FLOAT64) {
        const char* base = (const char*)view->data;
        const double* ptr = (const double*)(base + idx * view->strides[0]);
        *out = *ptr;
        return 1;
    } else if (view->element_type == DATARA_DTYPE_FLOAT32) {
        const char* base = (const char*)view->data;
        const float* ptr = (const float*)(base + idx * view->strides[0]);
        *out = (double)(*ptr);
        return 1;
    }
    return 0;
}

int64_t datara_memview_set_f64(DataraMemoryView* view, int64_t idx, double val) {
    if (!view || !view->data || idx < 0 || idx >= view->shape[0]) return 0;
    if (view->element_type == DATARA_DTYPE_FLOAT64) {
        char* base = (char*)view->data;
        double* ptr = (double*)(base + idx * view->strides[0]);
        *ptr = val;
        return 1;
    } else if (view->element_type == DATARA_DTYPE_FLOAT32) {
        char* base = (char*)view->data;
        float* ptr = (float*)(base + idx * view->strides[0]);
        *ptr = (float)val;
        return 1;
    }
    return 0;
}

int64_t datara_memview_get_i64(const DataraMemoryView* view, int64_t idx, int64_t* out) {
    if (!view || !view->data || !out || idx < 0 || idx >= view->shape[0]) return 0;
    const char* base = (const char*)view->data;
    const char* ptr = base + idx * view->strides[0];
    switch (view->element_type) {
        case DATARA_DTYPE_INT8:   *out = *(const int8_t*)ptr; return 1;
        case DATARA_DTYPE_UINT8:  *out = *(const uint8_t*)ptr; return 1;
        case DATARA_DTYPE_INT16:  *out = *(const int16_t*)ptr; return 1;
        case DATARA_DTYPE_UINT16: *out = *(const uint16_t*)ptr; return 1;
        case DATARA_DTYPE_INT32:  *out = *(const int32_t*)ptr; return 1;
        case DATARA_DTYPE_UINT32: *out = *(const uint32_t*)ptr; return 1;
        case DATARA_DTYPE_INT64:  *out = *(const int64_t*)ptr; return 1;
        case DATARA_DTYPE_UINT64: *out = (int64_t)*(const uint64_t*)ptr; return 1;
        default: return 0;
    }
}

int64_t datara_memview_set_i64(DataraMemoryView* view, int64_t idx, int64_t val) {
    if (!view || !view->data || idx < 0 || idx >= view->shape[0]) return 0;
    char* base = (char*)view->data;
    char* ptr = base + idx * view->strides[0];
    switch (view->element_type) {
        case DATARA_DTYPE_INT8:   *(int8_t*)ptr = (int8_t)val; return 1;
        case DATARA_DTYPE_UINT8:  *(uint8_t*)ptr = (uint8_t)val; return 1;
        case DATARA_DTYPE_INT16:  *(int16_t*)ptr = (int16_t)val; return 1;
        case DATARA_DTYPE_UINT16: *(uint16_t*)ptr = (uint16_t)val; return 1;
        case DATARA_DTYPE_INT32:  *(int32_t*)ptr = (int32_t)val; return 1;
        case DATARA_DTYPE_UINT32: *(uint32_t*)ptr = (uint32_t)val; return 1;
        case DATARA_DTYPE_INT64:  *(int64_t*)ptr = val; return 1;
        case DATARA_DTYPE_UINT64: *(uint64_t*)ptr = (uint64_t)val; return 1;
        default: return 0;
    }
}

// C FFI Flat Buffer Round-trip Test Helpers
int64_t datara_rt_test_list_f64_multiply(int64_t* list, double factor) {
    if (!list) return 0;
    DataraMemoryView mv = datara_memview_from_list_f64(list);
    for (int64_t i = 0; i < mv.shape[0]; ++i) {
        double v = 0.0;
        if (datara_memview_get_f64(&mv, i, &v)) {
            datara_memview_set_f64(&mv, i, v * factor);
        }
    }
    return mv.shape[0];
}

int64_t datara_rt_test_list_f64_sum(int64_t* list) {
    if (!list) return 0;
    DataraMemoryView mv = datara_memview_from_list_f64(list);
    double sum = 0.0;
    for (int64_t i = 0; i < mv.shape[0]; ++i) {
        double v = 0.0;
        if (datara_memview_get_f64(&mv, i, &v)) {
            sum += v;
        }
    }
    return (int64_t)sum;
}

static void test_custom_destructor(void* p) {
    int* counter = (int*)p;
    if (counter) (*counter)++;
}

int32_t datara_rt_polyglot_foundation_test(void) {
    // 1. Float round-trip
    DataraValue vf = datara_val_from_float(3.1415926535);
    if (!datara_val_is_float(vf)) return 0;
    if (fabs(datara_val_to_float(vf) - 3.1415926535) > 1e-9) return 0;

    // 2. Int round-trip
    DataraValue vi = datara_val_from_int(123456789LL);
    if (!datara_val_is_int(vi)) return 0;
    if (datara_val_to_int(vi) != 123456789LL) return 0;

    DataraValue vi_neg = datara_val_from_int(-42LL);
    if (!datara_val_is_int(vi_neg)) return 0;
    if (datara_val_to_int(vi_neg) != -42LL) return 0;

    // 3. Bool round-trip
    DataraValue vb_t = datara_val_from_bool(1);
    DataraValue vb_f = datara_val_from_bool(0);
    if (!datara_val_is_bool(vb_t) || datara_val_to_bool(vb_t) != 1) return 0;
    if (!datara_val_is_bool(vb_f) || datara_val_to_bool(vb_f) != 0) return 0;

    // 4. String pointer round-trip
    const char* test_str = "Datara Polyglot Engine";
    DataraValue vs = datara_val_from_str(test_str);
    if (!datara_val_is_str(vs)) return 0;
    if (strcmp(datara_val_to_str(vs), test_str) != 0) return 0;

    // 5. Raw pointer round-trip
    int dummy_target = 0x55AA;
    DataraValue vp = datara_val_from_rawptr(&dummy_target);
    if (!datara_val_is_rawptr(vp)) return 0;
    if (datara_val_to_rawptr(vp) != &dummy_target) return 0;

    // 6. Handle table & destructor test
    int dtor_called = 0;
    uint32_t h1 = datara_rt_handle_alloc(&dtor_called, "test_resource", test_custom_destructor);
    if (h1 == 0) return 0;
    if (datara_rt_handle_get(h1) != &dtor_called) return 0;
    if (strcmp(datara_rt_handle_tag(h1), "test_resource") != 0) return 0;
    if (!datara_rt_handle_free(h1)) return 0;
    if (dtor_called != 1) return 0;
    if (datara_rt_handle_get(h1) != NULL) return 0;

    // 7. MemoryView flat buffer roundtrip
    double raw_buf[4] = { 1.5, 2.5, 3.5, 4.5 };
    DataraMemoryView mv = datara_memview_1d(raw_buf, 4, DATARA_DTYPE_FLOAT64);
    if (mv.total_bytes != 4 * sizeof(double)) return 0;
    if (mv.ndim != 1 || mv.shape[0] != 4 || mv.strides[0] != sizeof(double)) return 0;
    double elem = 0.0;
    if (!datara_memview_get_f64(&mv, 2, &elem) || fabs(elem - 3.5) > 1e-9) return 0;
    if (!datara_memview_set_f64(&mv, 2, 99.0)) return 0;
    if (raw_buf[2] != 99.0) return 0;

    return 1;
}

