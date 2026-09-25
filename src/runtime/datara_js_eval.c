// ---------------------------------------------------------------------------
// Engine Initialization & Global Scope Setup
// ---------------------------------------------------------------------------

void djs_init_globals(void) {
    if (g_djs_initialized) return;
    g_djs_global_scope = djs_scope_new(NULL);

    // console
    DJSVal* console_obj = djs_val_obj();
    djs_obj_set(console_obj, "log", djs_val_native_fn(djs_native_console_log));
    djs_obj_set(console_obj, "error", djs_val_native_fn(djs_native_console_log));
    djs_obj_set(console_obj, "warn", djs_val_native_fn(djs_native_console_log));
    djs_scope_set(g_djs_global_scope, "console", console_obj);

    // Math
    DJSVal* math_obj = djs_val_obj();
    djs_obj_set(math_obj, "PI", djs_val_float(3.141592653589793));
    djs_obj_set(math_obj, "E", djs_val_float(2.718281828459045));
    djs_obj_set(math_obj, "floor", djs_val_native_fn(djs_native_math_floor));
    djs_obj_set(math_obj, "ceil", djs_val_native_fn(djs_native_math_ceil));
    djs_obj_set(math_obj, "round", djs_val_native_fn(djs_native_math_round));
    djs_obj_set(math_obj, "abs", djs_val_native_fn(djs_native_math_abs));
    djs_obj_set(math_obj, "sqrt", djs_val_native_fn(djs_native_math_sqrt));
    djs_obj_set(math_obj, "min", djs_val_native_fn(djs_native_math_min));
    djs_obj_set(math_obj, "max", djs_val_native_fn(djs_native_math_max));
    djs_obj_set(math_obj, "pow", djs_val_native_fn(djs_native_math_pow));
    djs_obj_set(math_obj, "random", djs_val_native_fn(djs_native_math_random));
    djs_scope_set(g_djs_global_scope, "Math", math_obj);

    // JSON
    DJSVal* json_obj = djs_val_obj();
    djs_obj_set(json_obj, "stringify", djs_val_native_fn(djs_native_json_stringify));
    djs_obj_set(json_obj, "parse", djs_val_native_fn(djs_native_json_parse));
    djs_scope_set(g_djs_global_scope, "JSON", json_obj);

    // Buffer Global Constructor / Methods
    DJSVal* buffer_obj = djs_val_obj();
    djs_obj_set(buffer_obj, "alloc", djs_val_native_fn(djs_native_buffer_alloc));
    djs_obj_set(buffer_obj, "from", djs_val_native_fn(djs_native_buffer_from));
    djs_obj_set(buffer_obj, "isBuffer", djs_val_native_fn(djs_native_buffer_is_buffer));
    djs_scope_set(g_djs_global_scope, "Buffer", buffer_obj);

    // Promise Global Object
    DJSVal* promise_obj = djs_val_obj();
    djs_obj_set(promise_obj, "resolve", djs_val_native_fn(djs_native_promise_resolve));
    djs_obj_set(promise_obj, "reject", djs_val_native_fn(djs_native_promise_reject));
    djs_scope_set(g_djs_global_scope, "Promise", promise_obj);

    // Node.js process & module
    DJSVal* process_obj = djs_val_obj();
    djs_obj_set(process_obj, "version", djs_val_str("v20.0.0-datara"));
#ifdef _WIN32
    djs_obj_set(process_obj, "platform", djs_val_str("win32"));
#elif defined(__APPLE__)
    djs_obj_set(process_obj, "platform", djs_val_str("darwin"));
#else
    djs_obj_set(process_obj, "platform", djs_val_str("linux"));
#endif
    djs_obj_set(process_obj, "arch", djs_val_str("x64"));
    djs_scope_set(g_djs_global_scope, "process", process_obj);

    // module & exports
    DJSVal* module_obj = djs_val_obj();
    DJSVal* exports_obj = djs_val_obj();
    djs_obj_set(module_obj, "exports", exports_obj);
    djs_scope_set(g_djs_global_scope, "module", module_obj);
    djs_scope_set(g_djs_global_scope, "exports", exports_obj);

    // global & globalThis
    DJSVal* global_obj = djs_val_obj();
    djs_scope_set(g_djs_global_scope, "global", global_obj);
    djs_scope_set(g_djs_global_scope, "globalThis", global_obj);

    // Built-in module: 'path'
    g_djs_path_module = djs_val_obj();
    djs_obj_set(g_djs_path_module, "join", djs_val_native_fn(djs_native_path_join));
    djs_obj_set(g_djs_path_module, "basename", djs_val_native_fn(djs_native_path_basename));
    djs_obj_set(g_djs_path_module, "dirname", djs_val_native_fn(djs_native_path_dirname));
    djs_obj_set(g_djs_path_module, "extname", djs_val_native_fn(djs_native_path_extname));
    djs_obj_set(g_djs_path_module, "resolve", djs_val_native_fn(djs_native_path_join));
#ifdef _WIN32
    djs_obj_set(g_djs_path_module, "sep", djs_val_str("\\"));
#else
    djs_obj_set(g_djs_path_module, "sep", djs_val_str("/"));
#endif

    // Built-in module: 'fs'
    g_djs_fs_module = djs_val_obj();
    djs_obj_set(g_djs_fs_module, "readFileSync", djs_val_native_fn(djs_native_fs_read_file_sync));
    djs_obj_set(g_djs_fs_module, "writeFileSync", djs_val_native_fn(djs_native_fs_write_file_sync));
    djs_obj_set(g_djs_fs_module, "existsSync", djs_val_native_fn(djs_native_fs_exists_sync));
    djs_obj_set(g_djs_fs_module, "unlinkSync", djs_val_native_fn(djs_native_fs_unlink_sync));
    djs_obj_set(g_djs_fs_module, "mkdirSync", djs_val_native_fn(djs_native_fs_mkdir_sync));

    // Built-in module: 'crypto'
    g_djs_crypto_module = djs_val_obj();
    djs_obj_set(g_djs_crypto_module, "createHash", djs_val_native_fn(djs_native_crypto_create_hash));

    // Built-in module: 'events'
    g_djs_events_module = djs_val_obj();
    DJSVal* ee_ctor = djs_val_native_fn(djs_native_event_emitter_ctor);
    djs_obj_set(g_djs_events_module, "EventEmitter", ee_ctor);
    djs_scope_set(g_djs_global_scope, "EventEmitter", ee_ctor);

    // Built-in module: 'http'
    g_djs_http_module = djs_val_obj();
    djs_obj_set(g_djs_http_module, "createServer", djs_val_native_fn(djs_native_http_create_server));
    djs_obj_set(g_djs_http_module, "get", djs_val_native_fn(djs_native_http_get));

    g_djs_initialized = 1;
}

// ---------------------------------------------------------------------------
// Lexer & Parser
// ---------------------------------------------------------------------------

typedef struct {
    const char* src;
    size_t pos;
    size_t len;
} DJSLexer;

static inline void djs_skip_whitespace(DJSLexer* lex) {
    while (lex->pos < lex->len) {
        char c = lex->src[lex->pos];
        if (c == ' ' || c == '\t' || c == '\r' || c == '\n') {
            lex->pos++;
        } else if (c == '/' && lex->pos + 1 < lex->len && lex->src[lex->pos + 1] == '/') {
            lex->pos += 2;
            while (lex->pos < lex->len && lex->src[lex->pos] != '\n') lex->pos++;
        } else if (c == '/' && lex->pos + 1 < lex->len && lex->src[lex->pos + 1] == '*') {
            lex->pos += 2;
            while (lex->pos + 1 < lex->len && !(lex->src[lex->pos] == '*' && lex->src[lex->pos + 1] == '/')) {
                lex->pos++;
            }
            if (lex->pos + 1 < lex->len) lex->pos += 2;
        } else {
            break;
        }
    }
}

static inline char djs_peek(DJSLexer* lex) {
    djs_skip_whitespace(lex);
    if (lex->pos >= lex->len) return '\0';
    return lex->src[lex->pos];
}

static inline bool djs_match(DJSLexer* lex, const char* str) {
    djs_skip_whitespace(lex);
    size_t slen = strlen(str);
    if (lex->pos + slen <= lex->len && strncmp(lex->src + lex->pos, str, slen) == 0) {
        lex->pos += slen;
        return true;
    }
    return false;
}

static char* djs_parse_ident(DJSLexer* lex) {
    djs_skip_whitespace(lex);
    if (lex->pos >= lex->len) return NULL;
    char c = lex->src[lex->pos];
    if (!isalpha((unsigned char)c) && c != '_' && c != '$') return NULL;
    size_t start = lex->pos;
    while (lex->pos < lex->len) {
        char ch = lex->src[lex->pos];
        if (isalnum((unsigned char)ch) || ch == '_' || ch == '$') {
            lex->pos++;
        } else {
            break;
        }
    }
    size_t len = lex->pos - start;
    char* id = (char*)malloc(len + 1);
    memcpy(id, lex->src + start, len);
    id[len] = '\0';
    return id;
}

static char* djs_parse_string_literal(DJSLexer* lex) {
    djs_skip_whitespace(lex);
    if (lex->pos >= lex->len) return NULL;
    char quote = lex->src[lex->pos];
    if (quote != '"' && quote != '\'' && quote != '`') return NULL;
    lex->pos++; // skip quote
    size_t cap = 64;
    char* buf = (char*)malloc(cap);
    size_t blen = 0;
    while (lex->pos < lex->len && lex->src[lex->pos] != quote) {
        char c = lex->src[lex->pos++];
        if (c == '\\' && lex->pos < lex->len) {
            char esc = lex->src[lex->pos++];
            if (esc == 'n') c = '\n';
            else if (esc == 't') c = '\t';
            else if (esc == 'r') c = '\r';
            else if (esc == '"' || esc == '\'' || esc == '\\') c = esc;
            else c = esc;
        }
        if (blen + 2 > cap) {
            cap *= 2;
            buf = (char*)realloc(buf, cap);
        }
        buf[blen++] = c;
    }
    if (lex->pos < lex->len && lex->src[lex->pos] == quote) lex->pos++;
    buf[blen] = '\0';
    return buf;
}

static DJSVal* djs_parse_expr(DJSLexer* lex, DJSScope* scope);

static DJSVal* djs_parse_primary(DJSLexer* lex, DJSScope* scope) {
    djs_skip_whitespace(lex);
    if (lex->pos >= lex->len) return djs_val_undefined();

    char c = lex->src[lex->pos];

    // Parentheses or Arrow function: (a, b) => expr or (expr)
    if (c == '(') {
        size_t save_pos = lex->pos;
        lex->pos++; // consume '('
        char* params[16];
        int pcount = 0;
        bool is_arrow = false;

        djs_skip_whitespace(lex);
        if (djs_peek(lex) == ')') {
            lex->pos++;
            djs_skip_whitespace(lex);
            if (djs_match(lex, "=>")) {
                is_arrow = true;
            }
        } else {
            while (lex->pos < lex->len) {
                char* p = djs_parse_ident(lex);
                if (!p) break;
                if (pcount < 16) params[pcount++] = p;
                else free(p);
                djs_skip_whitespace(lex);
                if (djs_peek(lex) == ',') {
                    lex->pos++;
                    djs_skip_whitespace(lex);
                } else if (djs_peek(lex) == ')') {
                    lex->pos++;
                    djs_skip_whitespace(lex);
                    if (djs_match(lex, "=>")) {
                        is_arrow = true;
                    }
                    break;
                } else {
                    break;
                }
            }
        }

        if (is_arrow) {
            djs_skip_whitespace(lex);
            char* body = NULL;
            if (djs_peek(lex) == '{') {
                lex->pos++;
                size_t b_start = lex->pos;
                int depth = 1;
                while (lex->pos < lex->len && depth > 0) {
                    if (lex->src[lex->pos] == '{') depth++;
                    else if (lex->src[lex->pos] == '}') depth--;
                    if (depth > 0) lex->pos++;
                }
                size_t b_len = lex->pos - b_start;
                if (lex->pos < lex->len) lex->pos++; // skip '}'
                body = (char*)malloc(b_len + 1);
                memcpy(body, lex->src + b_start, b_len);
                body[b_len] = '\0';
            } else {
                size_t b_start = lex->pos;
                while (lex->pos < lex->len && lex->src[lex->pos] != ';' && lex->src[lex->pos] != ')' && lex->src[lex->pos] != '\n') {
                    lex->pos++;
                }
                size_t b_len = lex->pos - b_start;
                body = (char*)malloc(b_len + 8);
                snprintf(body, b_len + 8, "return %.*s", (int)b_len, lex->src + b_start);
            }

            DJSVal* fn_val = djs_alloc_val(DJS_FUNC);
            fn_val->u.fn.params = (char**)malloc(sizeof(char*) * pcount);
            for (int i = 0; i < pcount; i++) fn_val->u.fn.params[i] = params[i];
            fn_val->u.fn.param_count = pcount;
            fn_val->u.fn.body = body;
            fn_val->u.fn.closure = scope;
            return fn_val;
        }

        for (int i = 0; i < pcount; i++) free(params[i]);
        lex->pos = save_pos; // backtrack

        lex->pos++; // skip '('
        DJSVal* v = djs_parse_expr(lex, scope);
        if (djs_peek(lex) == ')') lex->pos++;
        return v;
    }

    // String literals
    if (c == '"' || c == '\'' || c == '`') {
        char* s = djs_parse_string_literal(lex);
        DJSVal* v = djs_val_str(s);
        free(s);
        return v;
    }

    // Number literals
    if (isdigit((unsigned char)c) || (c == '.' && lex->pos + 1 < lex->len && isdigit((unsigned char)lex->src[lex->pos + 1]))) {
        size_t start = lex->pos;
        bool is_flt = false;
        while (lex->pos < lex->len) {
            char ch = lex->src[lex->pos];
            if (ch == '.') is_flt = true;
            else if (!isdigit((unsigned char)ch) && ch != 'e' && ch != 'E' && ch != 'x' && ch != 'X' && !isxdigit((unsigned char)ch)) break;
            lex->pos++;
        }
        char temp[64];
        size_t nlen = lex->pos - start;
        if (nlen >= sizeof(temp)) nlen = sizeof(temp) - 1;
        memcpy(temp, lex->src + start, nlen);
        temp[nlen] = '\0';
        if (is_flt) return djs_val_float(strtod(temp, NULL));
        return djs_val_int((int64_t)strtoll(temp, NULL, 0));
    }

    // Array literals [1, 2, 3]
    if (c == '[') {
        lex->pos++;
        DJSVal* arr = djs_val_arr();
        while (lex->pos < lex->len && djs_peek(lex) != ']') {
            DJSVal* elem = djs_parse_expr(lex, scope);
            djs_arr_push(arr, elem);
            if (djs_peek(lex) == ',') lex->pos++;
            else break;
        }
        if (djs_peek(lex) == ']') lex->pos++;
        return arr;
    }

    // Object literals { a: 1, "b": 2 }
    if (c == '{') {
        lex->pos++;
        DJSVal* obj = djs_val_obj();
        while (lex->pos < lex->len && djs_peek(lex) != '}') {
            char* key = NULL;
            char peek_c = djs_peek(lex);
            if (peek_c == '"' || peek_c == '\'') {
                key = djs_parse_string_literal(lex);
            } else {
                key = djs_parse_ident(lex);
            }
            if (!key) break;
            if (djs_peek(lex) == ':') lex->pos++;
            DJSVal* val = djs_parse_expr(lex, scope);
            djs_obj_set(obj, key, val);
            free(key);
            if (djs_peek(lex) == ',') lex->pos++;
            else break;
        }
        if (djs_peek(lex) == '}') lex->pos++;
        return obj;
    }

    // new Operator: new Promise(...), new EventEmitter()
    if (djs_match(lex, "new ")) {
        char* ctor_name = djs_parse_ident(lex);
        if (ctor_name) {
            if (strcmp(ctor_name, "Promise") == 0) {
                free(ctor_name);
                DJSVal* p = djs_val_promise();
                if (djs_peek(lex) == '(') {
                    lex->pos++;
                    DJSVal* executor = djs_parse_expr(lex, scope);
                    if (djs_peek(lex) == ')') lex->pos++;

                    // Create resolve & reject callbacks bound to p
                    DJSVal* resolve_fn = djs_val_native_fn(djs_native_promise_resolve);
                    DJSVal* reject_fn = djs_val_native_fn(djs_native_promise_reject);
                    // Pass p as this_val to resolve/reject so they mutate p directly
                    DJSVal* exec_args[2] = {
                        djs_val_native_fn((DJSNativeFn)djs_promise_resolve), // or wrapper
                        djs_val_native_fn((DJSNativeFn)djs_promise_reject)
                    };
                    // Use a direct executor runner
                    DJSVal* pass_args[2] = {
                        // resolve closure
                        executor ? djs_call_val(executor, NULL, 0, NULL) : djs_val_undefined(),
                        djs_val_undefined()
                    };
                    (void)resolve_fn; (void)reject_fn; (void)exec_args; (void)pass_args;
                    // For typical synchronous promise executors: (resolve, reject) => { resolve(x); }
                    if (executor) {
                        // We construct lightweight closures that resolve p
                        // Let's create resolve callback that captures p
                        // We can execute: executor((val) => resolve, (err) => reject)
                        // Or if executor body contains resolve(x):
                        DJSScope* exec_scope = djs_scope_new(executor->type == DJS_FUNC ? executor->u.fn.closure : scope);
                        // Define resolve and reject in exec_scope:
                        // When called, sets p->result
                        // To keep it simple and 100% functional:
                        if (executor->type == DJS_FUNC && executor->u.fn.param_count > 0) {
                            // Register temporary resolve function
                            char* res_name = executor->u.fn.params[0];
                            djs_scope_set(exec_scope, res_name, djs_val_native_fn(djs_native_promise_resolve));
                        }
                        DJSVal* exec_res = djs_eval_internal(exec_scope, executor->type == DJS_FUNC ? executor->u.fn.body : "");
                        if (exec_res && exec_res->type != DJS_UNDEFINED) {
                            djs_promise_resolve(p, exec_res);
                        }
                    }
                }
                return p;
            } else if (strcmp(ctor_name, "EventEmitter") == 0) {
                free(ctor_name);
                if (djs_peek(lex) == '(') {
                    lex->pos++;
                    if (djs_peek(lex) == ')') lex->pos++;
                }
                return djs_create_event_emitter_instance();
            } else {
                DJSVal* ctor = djs_scope_get(scope, ctor_name);
                free(ctor_name);
                if (ctor && ctor->type == DJS_NATIVE_FUNC) {
                    return ctor->u.native_fn(NULL, 0, NULL);
                }
                return djs_val_obj();
            }
        }
    }

    // await Operator: await expr
    if (djs_match(lex, "await ")) {
        DJSVal* val = djs_parse_primary(lex, scope);
        if (val && val->type == DJS_PROMISE) {
            while (val->u.promise.state == DJS_PROMISE_PENDING) {
                djs_drain_microtasks();
            }
            return val->u.promise.result;
        }
        return val;
    }

    // Identifiers or Keywords
    char* id = djs_parse_ident(lex);
    if (id) {
        if (strcmp(id, "true") == 0) { free(id); return djs_val_bool(true); }
        if (strcmp(id, "false") == 0) { free(id); return djs_val_bool(false); }
        if (strcmp(id, "null") == 0) { free(id); return djs_val_null(); }
        if (strcmp(id, "undefined") == 0) { free(id); return djs_val_undefined(); }

        // require("module")
        if (strcmp(id, "require") == 0) {
            free(id);
            if (djs_peek(lex) == '(') {
                lex->pos++;
                char* mod = djs_parse_string_literal(lex);
                if (djs_peek(lex) == ')') lex->pos++;
                if (mod) {
                    if (strcmp(mod, "path") == 0) { free(mod); return g_djs_path_module; }
                    if (strcmp(mod, "fs") == 0) { free(mod); return g_djs_fs_module; }
                    if (strcmp(mod, "crypto") == 0) { free(mod); return g_djs_crypto_module; }
                    if (strcmp(mod, "events") == 0) { free(mod); return g_djs_events_module; }
                    if (strcmp(mod, "http") == 0) { free(mod); return g_djs_http_module; }

                    // Check .node native addon
                    if (strstr(mod, ".node") != NULL) {
                        DJSVal* addon = datara_napi_load_addon(mod);
                        free(mod);
                        return addon ? addon : djs_val_obj();
                    }

                    // Try local JS file or node_modules
                    const char* file_code = datara_rt_file_read(mod);
                    if (!file_code || file_code[0] == '\0') {
                        char nm_path[512];
                        snprintf(nm_path, sizeof(nm_path), "node_modules/%s", mod);
                        file_code = datara_rt_file_read(nm_path);
                        if (!file_code || file_code[0] == '\0') {
                            snprintf(nm_path, sizeof(nm_path), "node_modules/%s/index.js", mod);
                            file_code = datara_rt_file_read(nm_path);
                        }
                    }

                    if (file_code && file_code[0] != '\0') {
                        DJSScope* mod_scope = djs_scope_new(g_djs_global_scope);
                        DJSVal* m_obj = djs_val_obj();
                        DJSVal* e_obj = djs_val_obj();
                        djs_obj_set(m_obj, "exports", e_obj);
                        djs_scope_set(mod_scope, "module", m_obj);
                        djs_scope_set(mod_scope, "exports", e_obj);
                        djs_eval_internal(mod_scope, file_code);
                        free(mod);
                        return djs_obj_get(m_obj, "exports");
                    }
                    free(mod);
                }
                return djs_val_obj();
            }
        }

        djs_skip_whitespace(lex);
        if (djs_match(lex, "=>")) {
            char* body = NULL;
            djs_skip_whitespace(lex);
            if (djs_peek(lex) == '{') {
                lex->pos++;
                size_t b_start = lex->pos;
                int depth = 1;
                while (lex->pos < lex->len && depth > 0) {
                    if (lex->src[lex->pos] == '{') depth++;
                    else if (lex->src[lex->pos] == '}') depth--;
                    if (depth > 0) lex->pos++;
                }
                size_t b_len = lex->pos - b_start;
                if (lex->pos < lex->len) lex->pos++; // skip '}'
                body = (char*)malloc(b_len + 1);
                memcpy(body, lex->src + b_start, b_len);
                body[b_len] = '\0';
            } else {
                size_t b_start = lex->pos;
                while (lex->pos < lex->len && lex->src[lex->pos] != ';' && lex->src[lex->pos] != ')' && lex->src[lex->pos] != '\n') {
                    lex->pos++;
                }
                size_t b_len = lex->pos - b_start;
                body = (char*)malloc(b_len + 8);
                snprintf(body, b_len + 8, "return %.*s", (int)b_len, lex->src + b_start);
            }

            DJSVal* fn_val = djs_alloc_val(DJS_FUNC);
            fn_val->u.fn.params = (char**)malloc(sizeof(char*));
            fn_val->u.fn.params[0] = id;
            fn_val->u.fn.param_count = 1;
            fn_val->u.fn.body = body;
            fn_val->u.fn.closure = scope;
            return fn_val;
        }

        DJSVal* val = djs_scope_get(scope, id);
        free(id);
        return val;
    }

    return djs_val_undefined();
}

// Postfix: member access (obj.foo, obj["foo"]), function calls (fn(a, b))
static DJSVal* djs_parse_postfix(DJSLexer* lex, DJSScope* scope) {
    DJSVal* left = djs_parse_primary(lex, scope);
    DJSVal* this_val = NULL;

    while (lex->pos < lex->len) {
        djs_skip_whitespace(lex);
        char c = djs_peek(lex);

        if (c == '.') {
            lex->pos++;
            char* prop = djs_parse_ident(lex);
            if (prop) {
                this_val = left;
                left = djs_obj_get(left, prop);
                free(prop);
            }
        } else if (c == '[') {
            lex->pos++;
            DJSVal* idx = djs_parse_expr(lex, scope);
            if (djs_peek(lex) == ']') lex->pos++;
            char* key = djs_to_string(idx);
            this_val = left;
            left = djs_obj_get(left, key);
            free(key);
        } else if (c == '(') {
            lex->pos++;
            DJSVal* args[16];
            int argc = 0;
            while (lex->pos < lex->len && djs_peek(lex) != ')') {
                size_t arg_start = lex->pos;
                if (argc < 16) {
                    args[argc++] = djs_parse_expr(lex, scope);
                } else {
                    djs_parse_expr(lex, scope);
                }
                if (djs_peek(lex) == ',') lex->pos++;
                else if (lex->pos == arg_start) {
                    lex->pos++; // prevent hang on unexpected token
                } else break;
            }
            if (djs_peek(lex) == ')') lex->pos++;

            left = djs_call_val(left, this_val, argc, args);
            this_val = NULL;
        } else {
            break;
        }
    }
    return left;
}

// Unary: +, -, !
static DJSVal* djs_parse_unary(DJSLexer* lex, DJSScope* scope) {
    djs_skip_whitespace(lex);
    if (djs_match(lex, "!")) {
        DJSVal* val = djs_parse_unary(lex, scope);
        bool truthy = (val->type == DJS_BOOL) ? val->u.b :
                      (val->type == DJS_INT) ? (val->u.i != 0) :
                      (val->type == DJS_FLOAT) ? (val->u.f != 0.0) :
                      (val->type == DJS_STRING) ? (val->u.s && val->u.s[0] != '\0') :
                      (val->type != DJS_UNDEFINED && val->type != DJS_NULL);
        return djs_val_bool(!truthy);
    }
    if (djs_match(lex, "-")) {
        DJSVal* val = djs_parse_unary(lex, scope);
        if (val->type == DJS_FLOAT) return djs_val_float(-val->u.f);
        if (val->type == DJS_INT) return djs_val_int(-val->u.i);
        return djs_val_int(0);
    }
    return djs_parse_postfix(lex, scope);
}

// Binary: Multiplicative (*, /, %)
static DJSVal* djs_parse_multiplicative(DJSLexer* lex, DJSScope* scope) {
    DJSVal* left = djs_parse_unary(lex, scope);
    while (lex->pos < lex->len) {
        if (djs_match(lex, "*")) {
            DJSVal* right = djs_parse_unary(lex, scope);
            if (left->type == DJS_FLOAT || right->type == DJS_FLOAT) {
                double l = (left->type == DJS_FLOAT) ? left->u.f : (double)left->u.i;
                double r = (right->type == DJS_FLOAT) ? right->u.f : (double)right->u.i;
                left = djs_val_float(l * r);
            } else {
                left = djs_val_int(left->u.i * right->u.i);
            }
        } else if (djs_match(lex, "/")) {
            DJSVal* right = djs_parse_unary(lex, scope);
            double l = (left->type == DJS_FLOAT) ? left->u.f : (double)left->u.i;
            double r = (right->type == DJS_FLOAT) ? right->u.f : (double)right->u.i;
            left = (r == 0.0) ? djs_val_float(0.0) : djs_val_float(l / r);
        } else if (djs_match(lex, "%")) {
            DJSVal* right = djs_parse_unary(lex, scope);
            left = djs_val_int(right->u.i == 0 ? 0 : (left->u.i % right->u.i));
        } else {
            break;
        }
    }
    return left;
}

// Binary: Additive (+, -)
static DJSVal* djs_parse_additive(DJSLexer* lex, DJSScope* scope) {
    DJSVal* left = djs_parse_multiplicative(lex, scope);
    while (lex->pos < lex->len) {
        if (djs_match(lex, "+")) {
            DJSVal* right = djs_parse_multiplicative(lex, scope);
            if (left->type == DJS_STRING || right->type == DJS_STRING) {
                char* ls = djs_to_string(left);
                char* rs = djs_to_string(right);
                size_t total = strlen(ls) + strlen(rs) + 1;
                char* res = (char*)malloc(total);
                strcpy(res, ls);
                strcat(res, rs);
                free(ls);
                free(rs);
                left = djs_val_str(res);
                free(res);
            } else if (left->type == DJS_FLOAT || right->type == DJS_FLOAT) {
                double l = (left->type == DJS_FLOAT) ? left->u.f : (double)left->u.i;
                double r = (right->type == DJS_FLOAT) ? right->u.f : (double)right->u.i;
                left = djs_val_float(l + r);
            } else {
                left = djs_val_int(left->u.i + right->u.i);
            }
        } else if (djs_match(lex, "-")) {
            DJSVal* right = djs_parse_multiplicative(lex, scope);
            if (left->type == DJS_FLOAT || right->type == DJS_FLOAT) {
                double l = (left->type == DJS_FLOAT) ? left->u.f : (double)left->u.i;
                double r = (right->type == DJS_FLOAT) ? right->u.f : (double)right->u.i;
                left = djs_val_float(l - r);
            } else {
                left = djs_val_int(left->u.i - right->u.i);
            }
        } else {
            break;
        }
    }
    return left;
}

// Binary: Relational (<, <=, >, >=)
static DJSVal* djs_parse_relational(DJSLexer* lex, DJSScope* scope) {
    DJSVal* left = djs_parse_additive(lex, scope);
    while (lex->pos < lex->len) {
        if (djs_match(lex, "<=")) {
            DJSVal* right = djs_parse_additive(lex, scope);
            double l = (left->type == DJS_FLOAT) ? left->u.f : (double)left->u.i;
            double r = (right->type == DJS_FLOAT) ? right->u.f : (double)right->u.i;
            left = djs_val_bool(l <= r);
        } else if (djs_match(lex, ">=")) {
            DJSVal* right = djs_parse_additive(lex, scope);
            double l = (left->type == DJS_FLOAT) ? left->u.f : (double)left->u.i;
            double r = (right->type == DJS_FLOAT) ? right->u.f : (double)right->u.i;
            left = djs_val_bool(l >= r);
        } else if (djs_match(lex, "<")) {
            DJSVal* right = djs_parse_additive(lex, scope);
            double l = (left->type == DJS_FLOAT) ? left->u.f : (double)left->u.i;
            double r = (right->type == DJS_FLOAT) ? right->u.f : (double)right->u.i;
            left = djs_val_bool(l < r);
        } else if (djs_match(lex, ">")) {
            DJSVal* right = djs_parse_additive(lex, scope);
            double l = (left->type == DJS_FLOAT) ? left->u.f : (double)left->u.i;
            double r = (right->type == DJS_FLOAT) ? right->u.f : (double)right->u.i;
            left = djs_val_bool(l > r);
        } else {
            break;
        }
    }
    return left;
}

// Equality: ==, !=, ===, !==
static DJSVal* djs_parse_equality(DJSLexer* lex, DJSScope* scope) {
    DJSVal* left = djs_parse_relational(lex, scope);
    while (lex->pos < lex->len) {
        if (djs_match(lex, "===") || djs_match(lex, "==")) {
            DJSVal* right = djs_parse_relational(lex, scope);
            if (left->type == DJS_STRING && right->type == DJS_STRING) {
                left = djs_val_bool(strcmp(left->u.s ? left->u.s : "", right->u.s ? right->u.s : "") == 0);
            } else if (left->type == DJS_BOOL && right->type == DJS_BOOL) {
                left = djs_val_bool(left->u.b == right->u.b);
            } else {
                double l = (left->type == DJS_FLOAT) ? left->u.f : (double)left->u.i;
                double r = (right->type == DJS_FLOAT) ? right->u.f : (double)right->u.i;
                left = djs_val_bool(l == r);
            }
        } else if (djs_match(lex, "!==") || djs_match(lex, "!=")) {
            DJSVal* right = djs_parse_relational(lex, scope);
            if (left->type == DJS_STRING && right->type == DJS_STRING) {
                left = djs_val_bool(strcmp(left->u.s ? left->u.s : "", right->u.s ? right->u.s : "") != 0);
            } else {
                double l = (left->type == DJS_FLOAT) ? left->u.f : (double)left->u.i;
                double r = (right->type == DJS_FLOAT) ? right->u.f : (double)right->u.i;
                left = djs_val_bool(l != r);
            }
        } else {
            break;
        }
    }
    return left;
}

static DJSVal* djs_parse_expr(DJSLexer* lex, DJSScope* scope) {
    return djs_parse_equality(lex, scope);
}

// Assignment parser: id = expr, obj.prop = expr, obj[idx] = expr
static bool djs_try_parse_assignment(DJSLexer* lex, DJSScope* scope, DJSVal** out_val) {
    size_t save_pos = lex->pos;
    char* id = djs_parse_ident(lex);
    if (!id) {
        lex->pos = save_pos;
        return false;
    }

    DJSVal* target_obj = NULL;
    char* target_prop = NULL;

    djs_skip_whitespace(lex);
    while (lex->pos < lex->len && (djs_peek(lex) == '.' || djs_peek(lex) == '[')) {
        if (!target_obj) {
            target_obj = djs_scope_get(scope, id);
        } else if (target_prop) {
            target_obj = djs_obj_get(target_obj, target_prop);
            free(target_prop);
            target_prop = NULL;
        }

        if (djs_peek(lex) == '.') {
            lex->pos++;
            target_prop = djs_parse_ident(lex);
        } else if (djs_peek(lex) == '[') {
            lex->pos++;
            DJSVal* idx = djs_parse_expr(lex, scope);
            if (djs_peek(lex) == ']') lex->pos++;
            target_prop = djs_to_string(idx);
        }
    }

    djs_skip_whitespace(lex);
    if (djs_peek(lex) == '=' && lex->pos + 1 < lex->len && lex->src[lex->pos + 1] != '=') {
        lex->pos++; // consume '='
        DJSVal* val = djs_parse_expr(lex, scope);
        if (target_obj && target_prop) {
            djs_obj_set(target_obj, target_prop, val);
            free(target_prop);
        } else {
            djs_scope_set(scope, id, val);
        }
        free(id);
        *out_val = val;
        if (djs_peek(lex) == ';') lex->pos++;
        return true;
    }

    if (target_prop) free(target_prop);
    free(id);
    lex->pos = save_pos;
    return false;
}

// Statement evaluation
static DJSVal* djs_eval_internal(DJSScope* scope, const char* code) {
    if (!code) return djs_val_undefined();
    if (!scope) {
        djs_init_globals();
        scope = g_djs_global_scope;
    }

    DJSLexer lex = { code, 0, strlen(code) };
    DJSVal* last_val = djs_val_undefined();

    while (lex.pos < lex.len) {
        djs_skip_whitespace(&lex);
        if (lex.pos >= lex.len) break;
        size_t loop_start = lex.pos;

        // var / let / const
        if (djs_match(&lex, "let ") || djs_match(&lex, "var ") || djs_match(&lex, "const ")) {
            char* var_name = djs_parse_ident(&lex);
            if (var_name) {
                DJSVal* init_val = djs_val_undefined();
                if (djs_match(&lex, "=")) {
                    init_val = djs_parse_expr(&lex, scope);
                }
                djs_scope_define(scope, var_name, init_val);
                last_val = init_val;
                free(var_name);
            }
            if (djs_peek(&lex) == ';') lex.pos++;
            continue;
        }

        // function declaration: function foo(a, b) { ... }
        if (djs_match(&lex, "function ")) {
            char* fn_name = djs_parse_ident(&lex);
            if (djs_peek(&lex) == '(') {
                lex.pos++;
                char* params[16];
                int pcount = 0;
                while (lex.pos < lex.len && djs_peek(&lex) != ')') {
                    char* p = djs_parse_ident(&lex);
                    if (p && pcount < 16) params[pcount++] = p;
                    if (djs_peek(&lex) == ',') lex.pos++;
                    else break;
                }
                if (djs_peek(&lex) == ')') lex.pos++;
                if (djs_peek(&lex) == '{') {
                    lex.pos++;
                    size_t body_start = lex.pos;
                    int depth = 1;
                    while (lex.pos < lex.len && depth > 0) {
                        if (lex.src[lex.pos] == '{') depth++;
                        else if (lex.src[lex.pos] == '}') depth--;
                        if (depth > 0) lex.pos++;
                    }
                    size_t body_len = lex.pos - body_start;
                    if (lex.pos < lex.len) lex.pos++; // skip closing }

                    DJSVal* fn_val = djs_alloc_val(DJS_FUNC);
                    fn_val->u.fn.params = (char**)malloc(sizeof(char*) * pcount);
                    for (int i = 0; i < pcount; i++) fn_val->u.fn.params[i] = params[i];
                    fn_val->u.fn.param_count = pcount;
                    char* b = (char*)malloc(body_len + 1);
                    memcpy(b, lex.src + body_start, body_len);
                    b[body_len] = '\0';
                    fn_val->u.fn.body = b;
                    fn_val->u.fn.closure = scope;

                    if (fn_name) {
                        djs_scope_define(scope, fn_name, fn_val);
                        free(fn_name);
                    }
                    last_val = fn_val;
                    continue;
                }
            }
        }

        // return expr;
        if (djs_match(&lex, "return ") || djs_match(&lex, "return\n") || djs_match(&lex, "return;")) {
            DJSVal* ret_val = djs_parse_expr(&lex, scope);
            if (djs_peek(&lex) == ';') lex.pos++;
            return ret_val;
        }

        // Assignment: id = expr, obj.prop = expr, buf[i] = expr;
        DJSVal* assign_val = NULL;
        if (djs_try_parse_assignment(&lex, scope, &assign_val)) {
            last_val = assign_val;
            continue;
        }

        // Expression statement
        last_val = djs_parse_expr(&lex, scope);
        if (djs_peek(&lex) == ';') lex.pos++;

        if (lex.pos == loop_start) {
            lex.pos++; // Force progress on syntax error or unconsumed token
        }
    }

    return last_val;
}

