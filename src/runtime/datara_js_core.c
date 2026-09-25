// ---------------------------------------------------------------------------
// Engine Global State
// ---------------------------------------------------------------------------

DJSScope* g_djs_global_scope = NULL;
static int g_djs_initialized = 0;

static DJSVal* g_djs_fs_module = NULL;
static DJSVal* g_djs_path_module = NULL;
static DJSVal* g_djs_crypto_module = NULL;
static DJSVal* g_djs_events_module = NULL;
static DJSVal* g_djs_http_module = NULL;

// Forward declarations
static DJSVal* djs_eval_internal(DJSScope* scope, const char* code);
static DJSVal* djs_call_val(DJSVal* fn, DJSVal* this_val, int argc, DJSVal** argv);

// ---------------------------------------------------------------------------
// Microtask Queue & Promise Internals
// ---------------------------------------------------------------------------

typedef struct DJSMicrotask {
    DJSVal* fn;
    DJSVal* arg;
    DJSVal* chained_promise;
    struct DJSMicrotask* next;
} DJSMicrotask;

static DJSMicrotask* g_microtask_head = NULL;
static DJSMicrotask* g_microtask_tail = NULL;

static void djs_enqueue_microtask(DJSVal* fn, DJSVal* arg, DJSVal* chained_promise) {
    if (!fn) return;
    DJSMicrotask* task = (DJSMicrotask*)malloc(sizeof(DJSMicrotask));
    if (!task) return;
    task->fn = fn;
    task->arg = arg;
    task->chained_promise = chained_promise;
    task->next = NULL;

    if (!g_microtask_tail) {
        g_microtask_head = task;
        g_microtask_tail = task;
    } else {
        g_microtask_tail->next = task;
        g_microtask_tail = task;
    }
}

static void djs_promise_resolve(DJSVal* promise, DJSVal* val);
static void djs_promise_reject(DJSVal* promise, DJSVal* reason);

static void djs_drain_microtasks(void) {
    while (g_microtask_head) {
        DJSMicrotask* task = g_microtask_head;
        g_microtask_head = task->next;
        if (!g_microtask_head) g_microtask_tail = NULL;

        DJSVal* argv[1] = { task->arg ? task->arg : djs_val_undefined() };
        DJSVal* res = djs_call_val(task->fn, NULL, 1, argv);

        if (task->chained_promise) {
            djs_promise_resolve(task->chained_promise, res ? res : djs_val_undefined());
        }
        free(task);
    }
}

static void djs_promise_resolve(DJSVal* p, DJSVal* val) {
    if (!p || p->type != DJS_PROMISE || p->u.promise.state != DJS_PROMISE_PENDING) return;
    p->u.promise.state = DJS_PROMISE_FULFILLED;
    p->u.promise.result = val ? val : djs_val_undefined();

    for (int i = 0; i < p->u.promise.then_count; i++) {
        djs_enqueue_microtask(p->u.promise.then_fns[i], p->u.promise.result, NULL);
    }
    p->u.promise.then_count = 0;
}

static void djs_promise_reject(DJSVal* p, DJSVal* reason) {
    if (!p || p->type != DJS_PROMISE || p->u.promise.state != DJS_PROMISE_PENDING) return;
    p->u.promise.state = DJS_PROMISE_REJECTED;
    p->u.promise.result = reason ? reason : djs_val_undefined();

    for (int i = 0; i < p->u.promise.catch_count; i++) {
        djs_enqueue_microtask(p->u.promise.catch_fns[i], p->u.promise.result, NULL);
    }
    p->u.promise.catch_count = 0;
}

static DJSVal* djs_promise_add_then(DJSVal* p, DJSVal* on_fulfilled, DJSVal* on_rejected) {
    if (!p || p->type != DJS_PROMISE) return djs_val_undefined();
    DJSVal* next_p = djs_val_promise();

    if (p->u.promise.state == DJS_PROMISE_FULFILLED) {
        if (on_fulfilled) {
            djs_enqueue_microtask(on_fulfilled, p->u.promise.result, next_p);
        } else {
            djs_promise_resolve(next_p, p->u.promise.result);
        }
    } else if (p->u.promise.state == DJS_PROMISE_REJECTED) {
        if (on_rejected) {
            djs_enqueue_microtask(on_rejected, p->u.promise.result, next_p);
        } else {
            djs_promise_reject(next_p, p->u.promise.result);
        }
    } else {
        // Pending: store callbacks
        if (on_fulfilled) {
            if (p->u.promise.then_count >= p->u.promise.then_cap) {
                p->u.promise.then_cap *= 2;
                p->u.promise.then_fns = (DJSVal**)realloc(p->u.promise.then_fns, sizeof(DJSVal*) * p->u.promise.then_cap);
            }
            p->u.promise.then_fns[p->u.promise.then_count++] = on_fulfilled;
        }
        if (on_rejected) {
            if (p->u.promise.catch_count >= p->u.promise.catch_cap) {
                p->u.promise.catch_cap *= 2;
                p->u.promise.catch_fns = (DJSVal**)realloc(p->u.promise.catch_fns, sizeof(DJSVal*) * p->u.promise.catch_cap);
            }
            p->u.promise.catch_fns[p->u.promise.catch_count++] = on_rejected;
        }
    }
    return next_p;
}

// ---------------------------------------------------------------------------
// Native Buffer Methods
// ---------------------------------------------------------------------------

static DJSVal* djs_native_buf_read_uint8(DJSVal* this_val, int argc, DJSVal** argv) {
    if (!this_val || this_val->type != DJS_BUFFER) return djs_val_int(0);
    size_t offset = (argc > 0 && argv[0]->type == DJS_INT) ? (size_t)argv[0]->u.i : 0;
    if (offset >= this_val->u.buffer.length) return djs_val_int(0);
    return djs_val_int((int64_t)this_val->u.buffer.data[offset]);
}

static DJSVal* djs_native_buf_write_uint8(DJSVal* this_val, int argc, DJSVal** argv) {
    if (!this_val || this_val->type != DJS_BUFFER || argc < 1) return djs_val_undefined();
    uint8_t byte_val = (uint8_t)(argv[0]->type == DJS_INT ? argv[0]->u.i : 0);
    size_t offset = (argc > 1 && argv[1]->type == DJS_INT) ? (size_t)argv[1]->u.i : 0;
    if (offset < this_val->u.buffer.length) {
        this_val->u.buffer.data[offset] = byte_val;
    }
    return djs_val_int((int64_t)(offset + 1));
}

static DJSVal* djs_native_buf_read_int32_le(DJSVal* this_val, int argc, DJSVal** argv) {
    if (!this_val || this_val->type != DJS_BUFFER) return djs_val_int(0);
    size_t offset = (argc > 0 && argv[0]->type == DJS_INT) ? (size_t)argv[0]->u.i : 0;
    if (offset + 4 > this_val->u.buffer.length) return djs_val_int(0);
    int32_t val;
    memcpy(&val, this_val->u.buffer.data + offset, 4);
    return djs_val_int((int64_t)val);
}

static DJSVal* djs_native_buf_write_int32_le(DJSVal* this_val, int argc, DJSVal** argv) {
    if (!this_val || this_val->type != DJS_BUFFER || argc < 1) return djs_val_undefined();
    int32_t val = (int32_t)(argv[0]->type == DJS_INT ? argv[0]->u.i : (int64_t)argv[0]->u.f);
    size_t offset = (argc > 1 && argv[1]->type == DJS_INT) ? (size_t)argv[1]->u.i : 0;
    if (offset + 4 <= this_val->u.buffer.length) {
        memcpy(this_val->u.buffer.data + offset, &val, 4);
    }
    return djs_val_int((int64_t)(offset + 4));
}

static DJSVal* djs_native_buf_read_double_le(DJSVal* this_val, int argc, DJSVal** argv) {
    if (!this_val || this_val->type != DJS_BUFFER) return djs_val_float(0.0);
    size_t offset = (argc > 0 && argv[0]->type == DJS_INT) ? (size_t)argv[0]->u.i : 0;
    if (offset + sizeof(double) > this_val->u.buffer.length) return djs_val_float(0.0);
    double val;
    memcpy(&val, this_val->u.buffer.data + offset, sizeof(double));
    return djs_val_float(val);
}

static DJSVal* djs_native_buf_write_double_le(DJSVal* this_val, int argc, DJSVal** argv) {
    if (!this_val || this_val->type != DJS_BUFFER || argc < 1) return djs_val_undefined();
    double val = (argv[0]->type == DJS_FLOAT) ? argv[0]->u.f : (double)argv[0]->u.i;
    size_t offset = (argc > 1 && argv[1]->type == DJS_INT) ? (size_t)argv[1]->u.i : 0;
    if (offset + sizeof(double) <= this_val->u.buffer.length) {
        memcpy(this_val->u.buffer.data + offset, &val, sizeof(double));
    }
    return djs_val_int((int64_t)(offset + sizeof(double)));
}

static DJSVal* djs_native_buf_to_string(DJSVal* this_val, int argc, DJSVal** argv) {
    (void)argc; (void)argv;
    if (!this_val || this_val->type != DJS_BUFFER) return djs_val_str("");
    char* s = (char*)malloc(this_val->u.buffer.length + 1);
    if (!s) return djs_val_str("");
    memcpy(s, this_val->u.buffer.data, this_val->u.buffer.length);
    s[this_val->u.buffer.length] = '\0';
    DJSVal* res = djs_val_str(s);
    free(s);
    return res;
}

// ---------------------------------------------------------------------------
// Native Promise Methods
// ---------------------------------------------------------------------------

static DJSVal* djs_native_promise_then(DJSVal* this_val, int argc, DJSVal** argv) {
    if (!this_val || this_val->type != DJS_PROMISE) return djs_val_undefined();
    DJSVal* on_fulfilled = argc > 0 ? argv[0] : NULL;
    DJSVal* on_rejected = argc > 1 ? argv[1] : NULL;
    return djs_promise_add_then(this_val, on_fulfilled, on_rejected);
}

static DJSVal* djs_native_promise_catch(DJSVal* this_val, int argc, DJSVal** argv) {
    if (!this_val || this_val->type != DJS_PROMISE) return djs_val_undefined();
    DJSVal* on_rejected = argc > 0 ? argv[0] : NULL;
    return djs_promise_add_then(this_val, NULL, on_rejected);
}

// ---------------------------------------------------------------------------
// Property Get / Set & Serialization
// ---------------------------------------------------------------------------

void djs_obj_set(DJSVal* obj, const char* key, DJSVal* val) {
    if (!obj || !key) return;

    if (obj->type == DJS_BUFFER) {
        char* end = NULL;
        long idx = strtol(key, &end, 10);
        if (end != key && idx >= 0 && (size_t)idx < obj->u.buffer.length) {
            uint8_t b = (uint8_t)(val->type == DJS_INT ? val->u.i :
                                  val->type == DJS_FLOAT ? (int64_t)val->u.f : 0);
            obj->u.buffer.data[idx] = b;
        }
        return;
    }

    if (obj->type != DJS_OBJECT) return;

    for (int i = 0; i < obj->u.o.count; i++) {
        if (strcmp(obj->u.o.props[i].key, key) == 0) {
            obj->u.o.props[i].val = val;
            return;
        }
    }
    if (obj->u.o.count >= obj->u.o.cap) {
        obj->u.o.cap *= 2;
        obj->u.o.props = (DJSProp*)realloc(obj->u.o.props, sizeof(DJSProp) * obj->u.o.cap);
    }
    obj->u.o.props[obj->u.o.count].key = djs_strdup(key);
    obj->u.o.props[obj->u.o.count].val = val;
    obj->u.o.count++;
}

DJSVal* djs_obj_get(DJSVal* obj, const char* key) {
    if (!obj || !key) return djs_val_undefined();

    if (obj->type == DJS_OBJECT) {
        for (int i = 0; i < obj->u.o.count; i++) {
            if (strcmp(obj->u.o.props[i].key, key) == 0) {
                return obj->u.o.props[i].val;
            }
        }
    } else if (obj->type == DJS_ARRAY) {
        if (strcmp(key, "length") == 0) {
            return djs_val_int(obj->u.a.count);
        }
        char* end = NULL;
        long idx = strtol(key, &end, 10);
        if (end != key && idx >= 0 && idx < obj->u.a.count) {
            return obj->u.a.items[idx];
        }
    } else if (obj->type == DJS_STRING) {
        if (strcmp(key, "length") == 0) {
            return djs_val_int((int64_t)strlen(obj->u.s ? obj->u.s : ""));
        }
    } else if (obj->type == DJS_BUFFER) {
        if (strcmp(key, "length") == 0) return djs_val_int((int64_t)obj->u.buffer.length);
        if (strcmp(key, "readUInt8") == 0) return djs_val_native_fn(djs_native_buf_read_uint8);
        if (strcmp(key, "writeUInt8") == 0) return djs_val_native_fn(djs_native_buf_write_uint8);
        if (strcmp(key, "readInt32LE") == 0) return djs_val_native_fn(djs_native_buf_read_int32_le);
        if (strcmp(key, "writeInt32LE") == 0) return djs_val_native_fn(djs_native_buf_write_int32_le);
        if (strcmp(key, "readDoubleLE") == 0) return djs_val_native_fn(djs_native_buf_read_double_le);
        if (strcmp(key, "writeDoubleLE") == 0) return djs_val_native_fn(djs_native_buf_write_double_le);
        if (strcmp(key, "toString") == 0) return djs_val_native_fn(djs_native_buf_to_string);

        char* end = NULL;
        long idx = strtol(key, &end, 10);
        if (end != key && idx >= 0 && (size_t)idx < obj->u.buffer.length) {
            return djs_val_int((int64_t)obj->u.buffer.data[idx]);
        }
    } else if (obj->type == DJS_PROMISE) {
        if (strcmp(key, "then") == 0) return djs_val_native_fn(djs_native_promise_then);
        if (strcmp(key, "catch") == 0) return djs_val_native_fn(djs_native_promise_catch);
    }
    return djs_val_undefined();
}

char* djs_to_string(DJSVal* v) {
    if (!v || v->type == DJS_UNDEFINED) return djs_strdup("undefined");
    if (v->type == DJS_NULL) return djs_strdup("null");
    if (v->type == DJS_BOOL) return djs_strdup(v->u.b ? "true" : "false");
    if (v->type == DJS_INT) {
        char buf[32];
        snprintf(buf, sizeof(buf), "%lld", (long long)v->u.i);
        return djs_strdup(buf);
    }
    if (v->type == DJS_FLOAT) {
        char buf[32];
        snprintf(buf, sizeof(buf), "%g", v->u.f);
        return djs_strdup(buf);
    }
    if (v->type == DJS_STRING) {
        return djs_strdup(v->u.s ? v->u.s : "");
    }
    if (v->type == DJS_BUFFER) {
        char* buf = (char*)malloc(v->u.buffer.length + 1);
        if (!buf) return djs_strdup("");
        memcpy(buf, v->u.buffer.data, v->u.buffer.length);
        buf[v->u.buffer.length] = '\0';
        return buf;
    }
    if (v->type == DJS_ARRAY) {
        size_t cap = 256;
        char* buf = (char*)malloc(cap);
        if (!buf) return djs_strdup("[]");
        buf[0] = '[';
        buf[1] = '\0';
        size_t len = 1;
        for (int i = 0; i < v->u.a.count; i++) {
            char* sub = djs_to_string(v->u.a.items[i]);
            size_t sub_len = strlen(sub);
            if (len + sub_len + 4 > cap) {
                cap = (len + sub_len + 4) * 2;
                buf = (char*)realloc(buf, cap);
            }
            if (i > 0) {
                strcat(buf, ",");
                len++;
            }
            strcat(buf, sub);
            len += sub_len;
            free(sub);
        }
        strcat(buf, "]");
        return buf;
    }
    if (v->type == DJS_OBJECT) {
        size_t cap = 256;
        char* buf = (char*)malloc(cap);
        if (!buf) return djs_strdup("{}");
        buf[0] = '{';
        buf[1] = '\0';
        size_t len = 1;
        for (int i = 0; i < v->u.o.count; i++) {
            char* sub = djs_to_string(v->u.o.props[i].val);
            size_t needed = strlen(v->u.o.props[i].key) + strlen(sub) + 8;
            if (len + needed > cap) {
                cap = (len + needed) * 2;
                buf = (char*)realloc(buf, cap);
            }
            if (i > 0) {
                strcat(buf, ",");
                len++;
            }
            snprintf(buf + len, cap - len, "\"%s\":%s", v->u.o.props[i].key, sub);
            len = strlen(buf);
            free(sub);
        }
        strcat(buf, "}");
        return buf;
    }
    if (v->type == DJS_FUNC || v->type == DJS_NATIVE_FUNC || v->type == DJS_NAPI_FUNC) {
        return djs_strdup("[Function]");
    }
    if (v->type == DJS_PROMISE) {
        return djs_strdup("[Promise]");
    }
    return djs_strdup("");
}

// ---------------------------------------------------------------------------
// Call Evaluator Helper
// ---------------------------------------------------------------------------

static DJSVal* djs_call_val(DJSVal* fn, DJSVal* this_val, int argc, DJSVal** argv) {
    if (!fn) return djs_val_undefined();

    if (fn->type == DJS_NATIVE_FUNC && fn->u.native_fn) {
        return fn->u.native_fn(this_val, argc, argv);
    }

    if (fn->type == DJS_NAPI_FUNC && fn->u.napi_fn.cb) {
        struct napi_callback_info__ info;
        info.argc = (size_t)argc;
        info.argv = (napi_value*)argv;
        info.this_arg = (napi_value)this_val;
        info.data = fn->u.napi_fn.data;
        napi_callback cb = (napi_callback)fn->u.napi_fn.cb;
        napi_env env = (napi_env)fn->u.napi_fn.env;
        napi_value r = cb(env, &info);
        return r ? (DJSVal*)r : djs_val_undefined();
    }

    if (fn->type == DJS_FUNC && fn->u.fn.body) {
        DJSScope* fn_scope = djs_scope_new(fn->u.fn.closure ? fn->u.fn.closure : g_djs_global_scope);
        for (int i = 0; i < fn->u.fn.param_count && i < argc; i++) {
            djs_scope_define(fn_scope, fn->u.fn.params[i], argv[i]);
        }
        return djs_eval_internal(fn_scope, fn->u.fn.body);
    }

    return djs_val_undefined();
}

// ---------------------------------------------------------------------------
// Standard SHA-256 Implementation (RFC 6234)
// ---------------------------------------------------------------------------

typedef struct {
    uint32_t state[8];
    uint64_t count;
    uint8_t buffer[64];
} DJS_SHA256_CTX;

#define DJS_ROTR(x, n) (((x) >> (n)) | ((x) << (32 - (n))))
#define DJS_CH(x, y, z) (((x) & (y)) ^ (~(x) & (z)))
#define DJS_MAJ(x, y, z) (((x) & (y)) ^ ((x) & (z)) ^ ((y) & (z)))
#define DJS_EP0(x) (DJS_ROTR(x, 2) ^ DJS_ROTR(x, 13) ^ DJS_ROTR(x, 22))
#define DJS_EP1(x) (DJS_ROTR(x, 6) ^ DJS_ROTR(x, 11) ^ DJS_ROTR(x, 25))
#define DJS_SIG0(x) (DJS_ROTR(x, 7) ^ DJS_ROTR(x, 18) ^ ((x) >> 3))
#define DJS_SIG1(x) (DJS_ROTR(x, 17) ^ DJS_ROTR(x, 19) ^ ((x) >> 10))

static const uint32_t K256[64] = {
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
};

static void djs_sha256_transform(DJS_SHA256_CTX* ctx, const uint8_t data[]) {
    uint32_t a, b, c, d, e, f, g, h, m[64];
    for (int i = 0, j = 0; i < 16; ++i, j += 4)
        m[i] = ((uint32_t)data[j] << 24) | ((uint32_t)data[j + 1] << 16) | ((uint32_t)data[j + 2] << 8) | ((uint32_t)data[j + 3]);
    for (int i = 16; i < 64; ++i)
        m[i] = DJS_SIG1(m[i - 2]) + m[i - 7] + DJS_SIG0(m[i - 15]) + m[i - 16];

    a = ctx->state[0]; b = ctx->state[1]; c = ctx->state[2]; d = ctx->state[3];
    e = ctx->state[4]; f = ctx->state[5]; g = ctx->state[6]; h = ctx->state[7];

    for (int i = 0; i < 64; ++i) {
        uint32_t t1 = h + DJS_EP1(e) + DJS_CH(e, f, g) + K256[i] + m[i];
        uint32_t t2 = DJS_EP0(a) + DJS_MAJ(a, b, c);
        h = g; g = f; f = e; e = d + t1;
        d = c; c = b; b = a; a = t1 + t2;
    }
    ctx->state[0] += a; ctx->state[1] += b; ctx->state[2] += c; ctx->state[3] += d;
    ctx->state[4] += e; ctx->state[5] += f; ctx->state[6] += g; ctx->state[7] += h;
}

static void djs_sha256_init(DJS_SHA256_CTX* ctx) {
    ctx->count = 0;
    ctx->state[0] = 0x6a09e667; ctx->state[1] = 0xbb67ae85;
    ctx->state[2] = 0x3c6ef372; ctx->state[3] = 0xa54ff53a;
    ctx->state[4] = 0x510e527f; ctx->state[5] = 0x9b05688c;
    ctx->state[6] = 0x1f83d9ab; ctx->state[7] = 0x5be0cd19;
}

static void djs_sha256_update(DJS_SHA256_CTX* ctx, const uint8_t* data, size_t len) {
    size_t i = 0;
    size_t idx = (size_t)((ctx->count >> 3) & 0x3f);
    ctx->count += ((uint64_t)len << 3);
    size_t part_len = 64 - idx;

    if (len >= part_len) {
        memcpy(&ctx->buffer[idx], data, part_len);
        djs_sha256_transform(ctx, ctx->buffer);
        for (i = part_len; i + 63 < len; i += 64)
            djs_sha256_transform(ctx, &data[i]);
        idx = 0;
    }
    memcpy(&ctx->buffer[idx], &data[i], len - i);
}

static void djs_sha256_final(DJS_SHA256_CTX* ctx, uint8_t hash[32]) {
    uint8_t final_count[8];
    for (int i = 0; i < 8; i++)
        final_count[i] = (uint8_t)((ctx->count >> ((7 - i) * 8)) & 0xff);

    uint8_t pad = 0x80;
    djs_sha256_update(ctx, &pad, 1);
    uint8_t zero = 0;
    while ((ctx->count & 0x1f8) != 0x1c0) {
        djs_sha256_update(ctx, &zero, 1);
    }
    djs_sha256_update(ctx, final_count, 8);

    for (int i = 0; i < 8; i++) {
        hash[i * 4]     = (uint8_t)((ctx->state[i] >> 24) & 0xff);
        hash[i * 4 + 1] = (uint8_t)((ctx->state[i] >> 16) & 0xff);
        hash[i * 4 + 2] = (uint8_t)((ctx->state[i] >> 8)  & 0xff);
        hash[i * 4 + 3] = (uint8_t)(ctx->state[i] & 0xff);
    }
}

