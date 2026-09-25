#include "datara_runtime.h"
#include "datara_js.h"
#include "datara_napi.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>
#include <math.h>
#include <ctype.h>

#ifdef _WIN32
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#include <winsock2.h>
#include <ws2tcpip.h>
#include <direct.h>
#define djs_mkdir(p) _mkdir(p)
#else
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <unistd.h>
#include <sys/stat.h>
#define closesocket close
#define SOCKET int
#define INVALID_SOCKET -1
#define SOCKET_ERROR -1
#define djs_mkdir(p) mkdir(p, 0755)
#endif


#include "datara_js_core.c"
#include "datara_js_modules.c"
#include "datara_js_eval.c"

// ---------------------------------------------------------------------------
// Exported C API Functions
// ---------------------------------------------------------------------------

const char* datara_js_eval(const char* code) {
    djs_init_globals();
    DJSVal* res = djs_eval_internal(g_djs_global_scope, code);
    return djs_to_string(res);
}

int64_t datara_js_eval_int(const char* code) {
    djs_init_globals();
    DJSVal* res = djs_eval_internal(g_djs_global_scope, code);
    if (!res) return 0;
    if (res->type == DJS_INT) return res->u.i;
    if (res->type == DJS_FLOAT) return (int64_t)res->u.f;
    if (res->type == DJS_BOOL) return res->u.b ? 1 : 0;
    if (res->type == DJS_STRING) return (int64_t)atoll(res->u.s ? res->u.s : "0");
    return 0;
}

double datara_js_eval_float(const char* code) {
    djs_init_globals();
    DJSVal* res = djs_eval_internal(g_djs_global_scope, code);
    if (!res) return 0.0;
    if (res->type == DJS_FLOAT) return res->u.f;
    if (res->type == DJS_INT) return (double)res->u.i;
    if (res->type == DJS_BOOL) return res->u.b ? 1.0 : 0.0;
    if (res->type == DJS_STRING) return atof(res->u.s ? res->u.s : "0.0");
    return 0.0;
}

int64_t datara_js_require(const char* module_name) {
    djs_init_globals();
    if (!module_name) return 0;

    if (strcmp(module_name, "path") == 0 ||
        strcmp(module_name, "fs") == 0 ||
        strcmp(module_name, "os") == 0 ||
        strcmp(module_name, "crypto") == 0 ||
        strcmp(module_name, "events") == 0 ||
        strcmp(module_name, "http") == 0 ||
        strcmp(module_name, "util") == 0) {
        return 1;
    }

    if (strstr(module_name, ".node") != NULL) {
        DJSVal* addon = datara_napi_load_addon(module_name);
        if (addon) {
            djs_scope_set(g_djs_global_scope, "addon", addon);
            return 1;
        }
        return 0;
    }

    const char* content = datara_rt_file_read(module_name);
    if (!content || content[0] == '\0') {
        char nm_path[512];
        snprintf(nm_path, sizeof(nm_path), "node_modules/%s", module_name);
        content = datara_rt_file_read(nm_path);
        if (!content || content[0] == '\0') {
            snprintf(nm_path, sizeof(nm_path), "node_modules/%s/index.js", module_name);
            content = datara_rt_file_read(nm_path);
        }
    }

    if (content && content[0] != '\0') {
        djs_eval_internal(g_djs_global_scope, content);
        return 1;
    }
    return 0;
}

const char* datara_js_call(const char* fn_name, const char* args_json) {
    djs_init_globals();
    if (!fn_name) return "null";

    DJSVal* fn = djs_scope_get(g_djs_global_scope, fn_name);
    if (!fn || (fn->type != DJS_FUNC && fn->type != DJS_NATIVE_FUNC && fn->type != DJS_NAPI_FUNC)) {
        return "null";
    }

    DJSVal* parsed_args = NULL;
    if (args_json && args_json[0] != '\0') {
        parsed_args = djs_eval_internal(NULL, args_json);
    }

    DJSVal* argv[16];
    int argc = 0;
    if (parsed_args && parsed_args->type == DJS_ARRAY) {
        argc = parsed_args->u.a.count;
        if (argc > 16) argc = 16;
        for (int i = 0; i < argc; i++) argv[i] = parsed_args->u.a.items[i];
    } else if (parsed_args && parsed_args->type != DJS_UNDEFINED) {
        argv[0] = parsed_args;
        argc = 1;
    }

    DJSVal* res = djs_call_val(fn, NULL, argc, argv);
    return djs_to_string(res);
}

const char* datara_js_call_0(const char* fn_name) {
    return datara_js_call(fn_name, "[]");
}

const char* datara_js_call_1(const char* fn_name, const char* a0) {
    if (!a0) return datara_js_call(fn_name, "[null]");
    size_t len = strlen(a0) + 4;
    char* b = (char*)malloc(len);
    if (!b) return "null";
    snprintf(b, len, "[%s]", a0);
    const char* r = datara_js_call(fn_name, b);
    free(b);
    return r;
}

const char* datara_js_call_2(const char* fn_name, const char* a0, const char* a1) {
    const char* s0 = a0 ? a0 : "null";
    const char* s1 = a1 ? a1 : "null";
    size_t len = strlen(s0) + strlen(s1) + 5;
    char* b = (char*)malloc(len);
    if (!b) return "null";
    snprintf(b, len, "[%s,%s]", s0, s1);
    const char* r = datara_js_call(fn_name, b);
    free(b);
    return r;
}

int64_t datara_js_set_global(const char* name, const char* json_val) {
    djs_init_globals();
    if (!name) return 0;
    DJSVal* v = (json_val && json_val[0] != '\0') ? djs_eval_internal(NULL, json_val) : djs_val_undefined();
    djs_scope_set(g_djs_global_scope, name, v);
    return 1;
}

const char* datara_js_get_global(const char* name) {
    djs_init_globals();
    if (!name) return "undefined";
    DJSVal* v = djs_scope_get(g_djs_global_scope, name);
    return djs_to_string(v);
}

// ---------------------------------------------------------------------------
// Zero-Copy DataraMemoryView Interop
// ---------------------------------------------------------------------------

int64_t datara_js_export_memview(const char* name, const DataraMemoryView* view) {
    djs_init_globals();
    if (!name || !view || !view->data) return 0;
    DJSVal* b = djs_val_buffer_wrap((uint8_t*)view->data, view->total_bytes, view);
    djs_scope_set(g_djs_global_scope, name, b);
    return 1;
}

int64_t datara_js_export_list_f64(const char* name, int64_t* list) {
    if (!list) return 0;
    DataraMemoryView mv = datara_memview_from_list_f64(list);
    return datara_js_export_memview(name, &mv);
}

int64_t datara_js_assert_same_ptr(const char* name, int64_t* list) {
    if (!name || !list) return 0;
    djs_init_globals();
    DJSVal* v = djs_scope_get(g_djs_global_scope, name);
    if (!v || v->type != DJS_BUFFER) return 0;
    DataraMemoryView mv = datara_memview_from_list_f64(list);
    return (v->u.buffer.data == mv.data) ? 1 : 0;
}
