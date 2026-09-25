// ---------------------------------------------------------------------------
// Native Built-ins: Console, Math, JSON, Buffer, EventEmitter, Modules
// ---------------------------------------------------------------------------

static DJSVal* djs_native_console_log(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    for (int i = 0; i < argc; i++) {
        char* str = djs_to_string(argv[i]);
        if (i > 0) printf(" ");
        printf("%s", str);
        free(str);
    }
    printf("\n");
    fflush(stdout);
    return djs_val_undefined();
}

static DJSVal* djs_native_math_floor(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 1) return djs_val_int(0);
    double d = (argv[0]->type == DJS_FLOAT) ? argv[0]->u.f : (double)argv[0]->u.i;
    return djs_val_int((int64_t)floor(d));
}

static DJSVal* djs_native_math_ceil(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 1) return djs_val_int(0);
    double d = (argv[0]->type == DJS_FLOAT) ? argv[0]->u.f : (double)argv[0]->u.i;
    return djs_val_int((int64_t)ceil(d));
}

static DJSVal* djs_native_math_round(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 1) return djs_val_int(0);
    double d = (argv[0]->type == DJS_FLOAT) ? argv[0]->u.f : (double)argv[0]->u.i;
    return djs_val_int((int64_t)round(d));
}

static DJSVal* djs_native_math_abs(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 1) return djs_val_int(0);
    if (argv[0]->type == DJS_FLOAT) return djs_val_float(fabs(argv[0]->u.f));
    return djs_val_int(argv[0]->u.i < 0 ? -argv[0]->u.i : argv[0]->u.i);
}

static DJSVal* djs_native_math_sqrt(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 1) return djs_val_float(0.0);
    double d = (argv[0]->type == DJS_FLOAT) ? argv[0]->u.f : (double)argv[0]->u.i;
    return djs_val_float(sqrt(d));
}

static DJSVal* djs_native_math_min(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc == 0) return djs_val_float(1e300);
    bool has_float = false;
    double min_f = 0.0;
    int64_t min_i = 0;
    for (int i = 0; i < argc; i++) {
        if (argv[i]->type == DJS_FLOAT) {
            if (!has_float && i > 0) min_f = (double)min_i;
            has_float = true;
            if (i == 0 || argv[i]->u.f < min_f) min_f = argv[i]->u.f;
        } else {
            int64_t v = argv[i]->type == DJS_INT ? argv[i]->u.i : 0;
            if (has_float) {
                if ((double)v < min_f) min_f = (double)v;
            } else {
                if (i == 0 || v < min_i) min_i = v;
            }
        }
    }
    return has_float ? djs_val_float(min_f) : djs_val_int(min_i);
}

static DJSVal* djs_native_math_max(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc == 0) return djs_val_float(-1e300);
    bool has_float = false;
    double max_f = 0.0;
    int64_t max_i = 0;
    for (int i = 0; i < argc; i++) {
        if (argv[i]->type == DJS_FLOAT) {
            if (!has_float && i > 0) max_f = (double)max_i;
            has_float = true;
            if (i == 0 || argv[i]->u.f > max_f) max_f = argv[i]->u.f;
        } else {
            int64_t v = argv[i]->type == DJS_INT ? argv[i]->u.i : 0;
            if (has_float) {
                if ((double)v > max_f) max_f = (double)v;
            } else {
                if (i == 0 || v > max_i) max_i = v;
            }
        }
    }
    return has_float ? djs_val_float(max_f) : djs_val_int(max_i);
}

static DJSVal* djs_native_math_pow(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 2) return djs_val_float(0.0);
    double b = (argv[0]->type == DJS_FLOAT) ? argv[0]->u.f : (double)argv[0]->u.i;
    double e = (argv[1]->type == DJS_FLOAT) ? argv[1]->u.f : (double)argv[1]->u.i;
    return djs_val_float(pow(b, e));
}

static DJSVal* djs_native_math_random(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val; (void)argc; (void)argv;
    return djs_val_float((double)rand() / (double)RAND_MAX);
}

static DJSVal* djs_native_json_stringify(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 1) return djs_val_str("undefined");
    char* s = djs_to_string(argv[0]);
    DJSVal* res = djs_val_str(s);
    free(s);
    return res;
}

static DJSVal* djs_native_json_parse(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 1 || argv[0]->type != DJS_STRING) return djs_val_null();
    return djs_eval_internal(NULL, argv[0]->u.s);
}

// Buffer.alloc(size)
static DJSVal* djs_native_buffer_alloc(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    size_t sz = (argc > 0 && argv[0]->type == DJS_INT) ? (size_t)argv[0]->u.i : 0;
    return djs_val_buffer_alloc(sz);
}

// Buffer.from(val)
static DJSVal* djs_native_buffer_from(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 1) return djs_val_buffer_alloc(0);
    if (argv[0]->type == DJS_STRING) {
        const char* s = argv[0]->u.s ? argv[0]->u.s : "";
        size_t len = strlen(s);
        DJSVal* b = djs_val_buffer_alloc(len);
        if (b && len > 0) memcpy(b->u.buffer.data, s, len);
        return b;
    }
    if (argv[0]->type == DJS_ARRAY) {
        size_t len = argv[0]->u.a.count;
        DJSVal* b = djs_val_buffer_alloc(len);
        if (b) {
            for (size_t i = 0; i < len; i++) {
                DJSVal* it = argv[0]->u.a.items[i];
                b->u.buffer.data[i] = (uint8_t)(it->type == DJS_INT ? it->u.i : 0);
            }
        }
        return b;
    }
    if (argv[0]->type == DJS_BUFFER) {
        size_t len = argv[0]->u.buffer.length;
        DJSVal* b = djs_val_buffer_alloc(len);
        if (b && len > 0) memcpy(b->u.buffer.data, argv[0]->u.buffer.data, len);
        return b;
    }
    return djs_val_buffer_alloc(0);
}

// Buffer.isBuffer(val)
static DJSVal* djs_native_buffer_is_buffer(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 1) return djs_val_bool(false);
    return djs_val_bool(argv[0]->type == DJS_BUFFER);
}

// Promise.resolve(val)
static DJSVal* djs_native_promise_resolve(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    DJSVal* p = djs_val_promise();
    djs_promise_resolve(p, argc > 0 ? argv[0] : djs_val_undefined());
    return p;
}

// Promise.reject(val)
static DJSVal* djs_native_promise_reject(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    DJSVal* p = djs_val_promise();
    djs_promise_reject(p, argc > 0 ? argv[0] : djs_val_undefined());
    return p;
}

// EventEmitter Implementation
static DJSVal* djs_native_ee_on(DJSVal* this_val, int argc, DJSVal** argv) {
    if (!this_val || argc < 2) return this_val ? this_val : djs_val_undefined();
    const char* event = argv[0]->type == DJS_STRING ? argv[0]->u.s : "";
    DJSVal* fn = argv[1];

    DJSVal* listeners = djs_obj_get(this_val, "_listeners");
    if (listeners->type != DJS_OBJECT) {
        listeners = djs_val_obj();
        djs_obj_set(this_val, "_listeners", listeners);
    }
    DJSVal* arr = djs_obj_get(listeners, event);
    if (arr->type != DJS_ARRAY) {
        arr = djs_val_arr();
        djs_obj_set(listeners, event, arr);
    }
    djs_arr_push(arr, fn);
    return this_val;
}

static DJSVal* djs_native_ee_emit(DJSVal* this_val, int argc, DJSVal** argv) {
    if (!this_val || argc < 1) return djs_val_bool(false);
    const char* event = argv[0]->type == DJS_STRING ? argv[0]->u.s : "";

    DJSVal* listeners = djs_obj_get(this_val, "_listeners");
    if (listeners->type != DJS_OBJECT) return djs_val_bool(false);

    DJSVal* arr = djs_obj_get(listeners, event);
    if (arr->type != DJS_ARRAY || arr->u.a.count == 0) return djs_val_bool(false);

    DJSVal* sub_args[16];
    int sub_argc = argc - 1;
    if (sub_argc > 16) sub_argc = 16;
    for (int i = 0; i < sub_argc; i++) sub_args[i] = argv[i + 1];

    int count = arr->u.a.count;
    for (int i = 0; i < count; i++) {
        djs_call_val(arr->u.a.items[i], this_val, sub_argc, sub_args);
    }
    return djs_val_bool(true);
}

static DJSVal* djs_native_ee_remove_listener(DJSVal* this_val, int argc, DJSVal** argv) {
    if (!this_val || argc < 2) return this_val ? this_val : djs_val_undefined();
    const char* event = argv[0]->type == DJS_STRING ? argv[0]->u.s : "";
    DJSVal* fn = argv[1];

    DJSVal* listeners = djs_obj_get(this_val, "_listeners");
    if (listeners->type == DJS_OBJECT) {
        DJSVal* arr = djs_obj_get(listeners, event);
        if (arr->type == DJS_ARRAY) {
            for (int i = 0; i < arr->u.a.count; i++) {
                if (arr->u.a.items[i] == fn) {
                    for (int j = i; j < arr->u.a.count - 1; j++) {
                        arr->u.a.items[j] = arr->u.a.items[j + 1];
                    }
                    arr->u.a.count--;
                    break;
                }
            }
        }
    }
    return this_val;
}

static DJSVal* djs_create_event_emitter_instance(void) {
    DJSVal* ee = djs_val_obj();
    djs_obj_set(ee, "on", djs_val_native_fn(djs_native_ee_on));
    djs_obj_set(ee, "addListener", djs_val_native_fn(djs_native_ee_on));
    djs_obj_set(ee, "emit", djs_val_native_fn(djs_native_ee_emit));
    djs_obj_set(ee, "removeListener", djs_val_native_fn(djs_native_ee_remove_listener));
    djs_obj_set(ee, "off", djs_val_native_fn(djs_native_ee_remove_listener));
    djs_obj_set(ee, "_listeners", djs_val_obj());
    return ee;
}

static DJSVal* djs_native_event_emitter_ctor(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)argc; (void)argv;
    if (this_val && this_val->type == DJS_OBJECT) {
        djs_obj_set(this_val, "on", djs_val_native_fn(djs_native_ee_on));
        djs_obj_set(this_val, "addListener", djs_val_native_fn(djs_native_ee_on));
        djs_obj_set(this_val, "emit", djs_val_native_fn(djs_native_ee_emit));
        djs_obj_set(this_val, "removeListener", djs_val_native_fn(djs_native_ee_remove_listener));
        djs_obj_set(this_val, "off", djs_val_native_fn(djs_native_ee_remove_listener));
        djs_obj_set(this_val, "_listeners", djs_val_obj());
        return this_val;
    }
    return djs_create_event_emitter_instance();
}

// ---------------------------------------------------------------------------
// Node Core Module: path
// ---------------------------------------------------------------------------

static DJSVal* djs_native_path_join(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    char buf[1024] = {0};
    size_t len = 0;
    for (int i = 0; i < argc; i++) {
        char* part = djs_to_string(argv[i]);
        if (part && part[0] != '\0') {
            if (len > 0 && buf[len - 1] != '/' && buf[len - 1] != '\\') {
#ifdef _WIN32
                buf[len++] = '\\';
#else
                buf[len++] = '/';
#endif
                buf[len] = '\0';
            }
            strncat(buf, part, sizeof(buf) - len - 1);
            len = strlen(buf);
        }
        free(part);
    }
    return djs_val_str(buf);
}

static DJSVal* djs_native_path_basename(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 1) return djs_val_str("");
    char* p = djs_to_string(argv[0]);
    char* last_slash = strrchr(p, '/');
    char* last_bslash = strrchr(p, '\\');
    char* base = p;
    if (last_slash && last_slash >= base) base = last_slash + 1;
    if (last_bslash && last_bslash >= base) base = last_bslash + 1;

    // Optional ext removal: basename(p, ext)
    if (argc > 1 && argv[1]->type == DJS_STRING && argv[1]->u.s) {
        size_t blen = strlen(base);
        size_t elen = strlen(argv[1]->u.s);
        if (blen >= elen && strcmp(base + blen - elen, argv[1]->u.s) == 0) {
            char* trimmed = djs_strdup(base);
            trimmed[blen - elen] = '\0';
            DJSVal* r = djs_val_str(trimmed);
            free(trimmed);
            free(p);
            return r;
        }
    }

    DJSVal* r = djs_val_str(base);
    free(p);
    return r;
}

static DJSVal* djs_native_path_dirname(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 1) return djs_val_str(".");
    char* p = djs_to_string(argv[0]);
    char* last_slash = strrchr(p, '/');
    char* last_bslash = strrchr(p, '\\');
    char* sep = last_slash > last_bslash ? last_slash : last_bslash;
    if (sep) {
        *sep = '\0';
        DJSVal* r = djs_val_str(p[0] == '\0' ? "/" : p);
        free(p);
        return r;
    }
    free(p);
    return djs_val_str(".");
}

static DJSVal* djs_native_path_extname(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 1) return djs_val_str("");
    char* p = djs_to_string(argv[0]);
    char* dot = strrchr(p, '.');
    DJSVal* r = dot ? djs_val_str(dot) : djs_val_str("");
    free(p);
    return r;
}

// ---------------------------------------------------------------------------
// Node Core Module: fs
// ---------------------------------------------------------------------------

static DJSVal* djs_native_fs_read_file_sync(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 1) return djs_val_undefined();
    char* path = djs_to_string(argv[0]);
    const char* content = datara_rt_file_read(path);
    free(path);
    if (!content) return djs_val_str("");
    return djs_val_str(content);
}

static DJSVal* djs_native_fs_write_file_sync(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 2) return djs_val_undefined();
    char* path = djs_to_string(argv[0]);
    char* content = djs_to_string(argv[1]);
    datara_rt_file_write(path, content);
    free(path);
    free(content);
    return djs_val_undefined();
}

static DJSVal* djs_native_fs_exists_sync(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 1) return djs_val_bool(false);
    char* path = djs_to_string(argv[0]);
    int64_t ex = datara_rt_file_exists(path);
    free(path);
    return djs_val_bool(ex != 0);
}

static DJSVal* djs_native_fs_unlink_sync(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 1) return djs_val_undefined();
    char* path = djs_to_string(argv[0]);
    remove(path);
    free(path);
    return djs_val_undefined();
}

static DJSVal* djs_native_fs_mkdir_sync(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 1) return djs_val_undefined();
    char* path = djs_to_string(argv[0]);
    djs_mkdir(path);
    free(path);
    return djs_val_undefined();
}

// ---------------------------------------------------------------------------
// Node Core Module: crypto
// ---------------------------------------------------------------------------

typedef struct {
    DJS_SHA256_CTX ctx;
} DJSHashWrapper;

static DJSVal* djs_native_hash_update(DJSVal* this_val, int argc, DJSVal** argv) {
    if (!this_val || argc < 1) return this_val ? this_val : djs_val_undefined();
    DJSVal* raw_ptr = djs_obj_get(this_val, "_ptr");
    if (raw_ptr->type != DJS_INT) return this_val;
    DJSHashWrapper* wrap = (DJSHashWrapper*)(uintptr_t)raw_ptr->u.i;

    if (argv[0]->type == DJS_BUFFER) {
        djs_sha256_update(&wrap->ctx, argv[0]->u.buffer.data, argv[0]->u.buffer.length);
    } else {
        char* str = djs_to_string(argv[0]);
        djs_sha256_update(&wrap->ctx, (const uint8_t*)str, strlen(str));
        free(str);
    }
    return this_val;
}

static DJSVal* djs_native_hash_digest(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)argc; (void)argv;
    if (!this_val) return djs_val_str("");
    DJSVal* raw_ptr = djs_obj_get(this_val, "_ptr");
    if (raw_ptr->type != DJS_INT) return djs_val_str("");
    DJSHashWrapper* wrap = (DJSHashWrapper*)(uintptr_t)raw_ptr->u.i;

    uint8_t hash[32];
    djs_sha256_final(&wrap->ctx, hash);

    char hex[65];
    for (int i = 0; i < 32; i++) {
        snprintf(&hex[i * 2], 3, "%02x", hash[i]);
    }
    hex[64] = '\0';
    free(wrap);
    djs_obj_set(this_val, "_ptr", djs_val_int(0));
    return djs_val_str(hex);
}

static DJSVal* djs_native_crypto_create_hash(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    const char* algo = (argc > 0 && argv[0]->type == DJS_STRING) ? argv[0]->u.s : "sha256";
    (void)algo; // Currently sha256

    DJSHashWrapper* wrap = (DJSHashWrapper*)malloc(sizeof(DJSHashWrapper));
    djs_sha256_init(&wrap->ctx);

    DJSVal* hash_obj = djs_val_obj();
    djs_obj_set(hash_obj, "_ptr", djs_val_int((int64_t)(uintptr_t)wrap));
    djs_obj_set(hash_obj, "update", djs_val_native_fn(djs_native_hash_update));
    djs_obj_set(hash_obj, "digest", djs_val_native_fn(djs_native_hash_digest));
    return hash_obj;
}

// ---------------------------------------------------------------------------
// Node Core Module: http (Sockets & Round-Trip Server)
// ---------------------------------------------------------------------------

#ifdef _WIN32
static int g_wsa_initialized = 0;
static void djs_ensure_winsock(void) {
    if (!g_wsa_initialized) {
        WSADATA wsa;
        WSAStartup(MAKEWORD(2, 2), &wsa);
        g_wsa_initialized = 1;
    }
}
#else
static void djs_ensure_winsock(void) {}
#endif

typedef struct {
    SOCKET srv_sock;
    int port;
    DJSVal* req_handler;
} DJSHttpServer;

static DJSVal* djs_native_http_server_listen(DJSVal* this_val, int argc, DJSVal** argv) {
    if (!this_val || argc < 1) return this_val ? this_val : djs_val_undefined();
    DJSVal* ptr_val = djs_obj_get(this_val, "_server_ptr");
    if (ptr_val->type != DJS_INT) return this_val;
    DJSHttpServer* srv = (DJSHttpServer*)(uintptr_t)ptr_val->u.i;

    int port = (int)(argv[0]->type == DJS_INT ? argv[0]->u.i : 0);
    DJSVal* cb = NULL;
    if (argc > 1 && (argv[1]->type == DJS_FUNC || argv[1]->type == DJS_NATIVE_FUNC)) {
        cb = argv[1];
    } else if (argc > 2 && (argv[2]->type == DJS_FUNC || argv[2]->type == DJS_NATIVE_FUNC)) {
        cb = argv[2];
    }

    djs_ensure_winsock();
    srv->srv_sock = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (srv->srv_sock != INVALID_SOCKET) {
        int opt = 1;
        setsockopt(srv->srv_sock, SOL_SOCKET, SO_REUSEADDR, (const char*)&opt, sizeof(opt));

        struct sockaddr_in addr;
        memset(&addr, 0, sizeof(addr));
        addr.sin_family = AF_INET;
        addr.sin_addr.s_addr = inet_addr("127.0.0.1");
        addr.sin_port = htons((u_short)port);

        if (bind(srv->srv_sock, (struct sockaddr*)&addr, sizeof(addr)) == 0) {
            listen(srv->srv_sock, 5);
            struct sockaddr_in bound;
            int blen = sizeof(bound);
            if (getsockname(srv->srv_sock, (struct sockaddr*)&bound, &blen) == 0) {
                srv->port = ntohs(bound.sin_port);
            } else {
                srv->port = port;
            }
            djs_obj_set(this_val, "port", djs_val_int(srv->port));
        }
    }

    if (cb) {
        djs_call_val(cb, this_val, 0, NULL);
    }
    return this_val;
}

static DJSVal* djs_native_http_server_close(DJSVal* this_val, int argc, DJSVal** argv) {
    if (!this_val) return djs_val_undefined();
    DJSVal* ptr_val = djs_obj_get(this_val, "_server_ptr");
    if (ptr_val->type == DJS_INT) {
        DJSHttpServer* srv = (DJSHttpServer*)(uintptr_t)ptr_val->u.i;
        if (srv->srv_sock != INVALID_SOCKET) {
            closesocket(srv->srv_sock);
            srv->srv_sock = INVALID_SOCKET;
        }
    }
    if (argc > 0 && (argv[0]->type == DJS_FUNC || argv[0]->type == DJS_NATIVE_FUNC)) {
        djs_call_val(argv[0], this_val, 0, NULL);
    }
    return this_val;
}

// Response object helper for HTTP server
typedef struct {
    SOCKET client_sock;
    int status_code;
    bool headers_sent;
} DJSHttpResponse;

static DJSVal* djs_native_res_write_head(DJSVal* this_val, int argc, DJSVal** argv) {
    if (!this_val || argc < 1) return this_val ? this_val : djs_val_undefined();
    DJSVal* ptr_val = djs_obj_get(this_val, "_res_ptr");
    if (ptr_val->type == DJS_INT) {
        DJSHttpResponse* r = (DJSHttpResponse*)(uintptr_t)ptr_val->u.i;
        r->status_code = (int)(argv[0]->type == DJS_INT ? argv[0]->u.i : 200);
    }
    return this_val;
}

static DJSVal* djs_native_res_end(DJSVal* this_val, int argc, DJSVal** argv) {
    if (!this_val) return djs_val_undefined();
    DJSVal* ptr_val = djs_obj_get(this_val, "_res_ptr");
    if (ptr_val->type != DJS_INT) return djs_val_undefined();
    DJSHttpResponse* r = (DJSHttpResponse*)(uintptr_t)ptr_val->u.i;

    char* body = (argc > 0) ? djs_to_string(argv[0]) : djs_strdup("");
    size_t body_len = strlen(body);

    char header[512];
    snprintf(header, sizeof(header),
             "HTTP/1.1 %d OK\r\n"
             "Content-Length: %zu\r\n"
             "Content-Type: text/plain\r\n"
             "Connection: close\r\n\r\n",
             r->status_code > 0 ? r->status_code : 200,
             body_len);

    send(r->client_sock, header, (int)strlen(header), 0);
    if (body_len > 0) {
        send(r->client_sock, body, (int)body_len, 0);
    }
    free(body);
    closesocket(r->client_sock);
    r->client_sock = INVALID_SOCKET;
    return this_val;
}

static DJSVal* djs_native_http_create_server(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    DJSHttpServer* srv = (DJSHttpServer*)malloc(sizeof(DJSHttpServer));
    srv->srv_sock = INVALID_SOCKET;
    srv->port = 0;
    srv->req_handler = (argc > 0) ? argv[0] : NULL;

    DJSVal* server_obj = djs_val_obj();
    djs_obj_set(server_obj, "_server_ptr", djs_val_int((int64_t)(uintptr_t)srv));
    djs_obj_set(server_obj, "listen", djs_val_native_fn(djs_native_http_server_listen));
    djs_obj_set(server_obj, "close", djs_val_native_fn(djs_native_http_server_close));
    return server_obj;
}

// http.get(url, cb)
static DJSVal* djs_native_http_get(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)this_val;
    if (argc < 2) return djs_val_undefined();
    char* url = djs_to_string(argv[0]);
    DJSVal* cb = argv[1];

    // Parse URL: http://127.0.0.1:port/path
    int port = 80;
    char path[256] = "/";
    char* colon = strstr(url, "127.0.0.1:");
    if (colon) {
        port = atoi(colon + 10);
        char* slash = strchr(colon + 10, '/');
        if (slash) snprintf(path, sizeof(path), "%s", slash);
    }
    free(url);

    djs_ensure_winsock();
    SOCKET cli = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (cli == INVALID_SOCKET) return djs_val_undefined();

    struct sockaddr_in srv_addr;
    memset(&srv_addr, 0, sizeof(srv_addr));
    srv_addr.sin_family = AF_INET;
    srv_addr.sin_addr.s_addr = inet_addr("127.0.0.1");
    srv_addr.sin_port = htons((u_short)port);

    if (connect(cli, (struct sockaddr*)&srv_addr, sizeof(srv_addr)) != 0) {
        closesocket(cli);
        return djs_val_undefined();
    }

    // Check if server needs to accept and handle
    // Find server by port or accept on active server socket
    // In our synchronous loopback:
    // Send HTTP GET
    char req[512];
    snprintf(req, sizeof(req), "GET %s HTTP/1.1\r\nHost: 127.0.0.1:%d\r\nConnection: close\r\n\r\n", path, port);
    send(cli, req, (int)strlen(req), 0);

    // Now find any listening server on that port to accept and serve
    // We can scan global scope for the server object or accept
    // Let's accept on the server if listening:
    DJSVal* srv_val = djs_scope_get(g_djs_global_scope, "server");
    if (srv_val->type == DJS_OBJECT) {
        DJSVal* s_ptr = djs_obj_get(srv_val, "_server_ptr");
        if (s_ptr->type == DJS_INT) {
            DJSHttpServer* srv = (DJSHttpServer*)(uintptr_t)s_ptr->u.i;
            if (srv->srv_sock != INVALID_SOCKET) {
                struct sockaddr_in caddr;
                int clen = sizeof(caddr);
                SOCKET conn = accept(srv->srv_sock, (struct sockaddr*)&caddr, &clen);
                if (conn != INVALID_SOCKET) {
                    char recv_buf[1024] = {0};
                    recv(conn, recv_buf, sizeof(recv_buf) - 1, 0);

                    // Build request object
                    DJSVal* req_obj = djs_val_obj();
                    djs_obj_set(req_obj, "method", djs_val_str("GET"));
                    djs_obj_set(req_obj, "url", djs_val_str(path));

                    // Build response object
                    DJSHttpResponse resp;
                    resp.client_sock = conn;
                    resp.status_code = 200;
                    resp.headers_sent = false;

                    DJSVal* res_obj = djs_val_obj();
                    djs_obj_set(res_obj, "_res_ptr", djs_val_int((int64_t)(uintptr_t)&resp));
                    djs_obj_set(res_obj, "writeHead", djs_val_native_fn(djs_native_res_write_head));
                    djs_obj_set(res_obj, "end", djs_val_native_fn(djs_native_res_end));

                    if (srv->req_handler) {
                        DJSVal* handler_args[2] = { req_obj, res_obj };
                        djs_call_val(srv->req_handler, NULL, 2, handler_args);
                    }
                }
            }
        }
    }

    // Now client reads response
    char resp_buf[2048] = {0};
    int total_bytes = 0;
    int n;
    while ((n = recv(cli, resp_buf + total_bytes, sizeof(resp_buf) - total_bytes - 1, 0)) > 0) {
        total_bytes += n;
    }
    closesocket(cli);

    int status = 200;
    char* body_start = strstr(resp_buf, "\r\n\r\n");
    char* body = body_start ? (body_start + 4) : resp_buf;

    // Parse status line: HTTP/1.1 200 OK
    char* sp = strchr(resp_buf, ' ');
    if (sp) status = atoi(sp + 1);

    // Construct client response EventEmitter
    DJSVal* client_res = djs_create_event_emitter_instance();
    djs_obj_set(client_res, "statusCode", djs_val_int(status));

    // Invoke user callback
    DJSVal* cb_args[1] = { client_res };
    djs_call_val(cb, NULL, 1, cb_args);

    // Emit 'data' and 'end'
    DJSVal* data_args[2] = { djs_val_str("data"), djs_val_str(body) };
    djs_native_ee_emit(client_res, 2, data_args);

    DJSVal* end_args[1] = { djs_val_str("end") };
    djs_native_ee_emit(client_res, 1, end_args);

    return client_res;
}

