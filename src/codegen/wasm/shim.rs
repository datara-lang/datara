use super::WasmEmitter;
use super::types::*;

impl WasmEmitter {
    /// Generates the companion JavaScript runtime loader with linear memory bump allocator,
    /// standard collection built-ins, and compositional capability bridging.
    pub(crate) fn generate_js_runtime_shim(
        module_name: &str,
        import_entries: &[(&'static str, &'static str, WasmFuncType, u32)],
    ) -> String {
        let has_fs = import_entries
            .iter()
            .any(|(m, _, _, _)| *m == "datara:fs@1.0");
        let has_net = import_entries
            .iter()
            .any(|(m, _, _, _)| *m == "datara:net@1.0");
        let has_sys = import_entries
            .iter()
            .any(|(m, _, _, _)| *m == "datara:sys@1.0");

        format!(
            r#"// Datara Capability-Native WebAssembly Runtime Loader ({module_name})
// Universal loader compatible with Node.js and Browser / Web Worker environments.

export async function loadDataraModule(wasmPath, customImports = {{}}) {{
    const wasmFile = wasmPath || './{module_name}.wasm';
    let wasmBytes;
    if (typeof process !== 'undefined' && process.versions != null && process.versions.node != null) {{
        const fs = await import('fs');
        wasmBytes = fs.readFileSync(wasmFile);
    }} else if (typeof fetch !== 'undefined') {{
        const resp = await fetch(wasmFile);
        wasmBytes = await resp.arrayBuffer();
    }} else {{
        throw new Error('Unsupported runtime: neither Node.js fs nor Browser fetch is available');
    }}

    // Linear-memory bump allocator and runtime context
    let memoryInstance = null;
    let bumpPointer = 65536; // start at 64KB boundary

    const readString = (ptr) => {{
        if (!memoryInstance || ptr === 0n || ptr === 0) return "";
        const p = Number(ptr);
        const mem = new Uint8Array(memoryInstance.buffer);
        const view = new DataView(memoryInstance.buffer);
        const len = view.getUint32(p, true);
        const bytes = mem.slice(p + 4, p + 4 + len);
        return new TextDecoder('utf-8').decode(bytes);
    }};

    const allocateMemory = (size) => {{
        const alignedSize = (Number(size) + 7) & ~7;
        const ptr = bumpPointer;
        bumpPointer += alignedSize;
        if (memoryInstance && bumpPointer > memoryInstance.buffer.byteLength) {{
            const neededPages = Math.ceil((bumpPointer - memoryInstance.buffer.byteLength) / 65536);
            memoryInstance.grow(neededPages);
        }}
        return BigInt(ptr);
    }};

    // In-memory collection storage backed by linear memory pointers
    const listStorage = new Map();
    let nextListHandle = 10000n;

    const mapStorage = new Map();
    let nextMapHandle = 20000n;

    // In-memory DOM storage for reactive WebAssembly UI
    const domStorage = new Map();
    let nextDomHandle = 30000n;

    // Ownership guard reference-counting storage (per-value Map)
    const ownershipStorage = new Map();

    const importObject = {{
        "datara:rt": {{
            alloc: (size) => allocateMemory(size),
            print: (val) => {{
                if (typeof val === 'bigint') {{
                    const buf = new ArrayBuffer(8);
                    const view = new DataView(buf);
                    view.setBigInt64(0, val, true);
                    const f = view.getFloat64(0, true);
                    const absVal = val < 0n ? -val : val;
                    if (Number.isFinite(f) && absVal > 4000000000000000n && Math.abs(f) > 1e-15 && Math.abs(f) < 1e15) {{
                        console.log(f);
                        return;
                    }}
                    console.log(val.toString());
                }} else {{
                    console.log(val);
                }}
            }},
            err: (val) => {{
                if (typeof val === 'bigint') {{
                    console.error(val.toString());
                }} else {{
                    console.error(val);
                }}
            }},
            list_create: (cap) => {{
                const handle = nextListHandle++;
                listStorage.set(handle, []);
                return handle;
            }},
            list_create_1: (a) => {{
                const handle = nextListHandle++;
                listStorage.set(handle, [a]);
                return handle;
            }},
            list_create_2: (a, b) => {{
                const handle = nextListHandle++;
                listStorage.set(handle, [a, b]);
                return handle;
            }},
            list_create_3: (a, b, c) => {{
                const handle = nextListHandle++;
                listStorage.set(handle, [a, b, c]);
                return handle;
            }},
            list_create_4: (a, b, c, d) => {{
                const handle = nextListHandle++;
                listStorage.set(handle, [a, b, c, d]);
                return handle;
            }},
            list_create_5: (a, b, c, d, e) => {{
                const handle = nextListHandle++;
                listStorage.set(handle, [a, b, c, d, e]);
                return handle;
            }},
            list_create_repeat: (elem, count) => {{
                const handle = nextListHandle++;
                const arr = new Array(Number(count)).fill(elem);
                listStorage.set(handle, arr);
                return handle;
            }},
            list_append: (listPtr, val) => {{
                let l = listStorage.get(listPtr);
                if (!l) {{ l = []; listStorage.set(listPtr, l); }}
                l.push(val);
                return listPtr;
            }},
            list_push: (listPtr, val) => {{
                let l = listStorage.get(listPtr);
                if (!l) {{ l = []; listStorage.set(listPtr, l); }}
                l.push(val);
                return listPtr;
            }},
            list_get: (listPtr, idx) => {{
                const l = listStorage.get(listPtr);
                if (!l || Number(idx) >= l.length) return 0n;
                return l[Number(idx)];
            }},
            list_set: (listPtr, idx, val) => {{
                let l = listStorage.get(listPtr);
                if (!l) {{ l = []; listStorage.set(listPtr, l); }}
                l[Number(idx)] = val;
                return val;
            }},
            list_len: (listPtr) => {{
                const l = listStorage.get(listPtr);
                return BigInt(l ? l.length : 0);
            }},
            map_create: () => {{
                const handle = nextMapHandle++;
                mapStorage.set(handle, new Map());
                return handle;
            }},
            map_insert: (mapPtr, key, val) => {{
                let m = mapStorage.get(mapPtr);
                if (!m) {{ m = new Map(); mapStorage.set(mapPtr, m); }}
                m.set(key, val);
                return val;
            }},
            map_set: (mapPtr, key, val) => {{
                let m = mapStorage.get(mapPtr);
                if (!m) {{ m = new Map(); mapStorage.set(mapPtr, m); }}
                m.set(key, val);
                return val;
            }},
            map_get: (mapPtr, key) => {{
                const m = mapStorage.get(mapPtr);
                return (m && m.has(key)) ? m.get(key) : 0n;
            }},
            map_len: (mapPtr) => {{
                const m = mapStorage.get(mapPtr);
                return BigInt(m ? m.size : 0);
            }},
            own_acquire: (val) => {{
                const count = (ownershipStorage.get(val) || 0) + 1;
                ownershipStorage.set(val, count);
                return val;
            }},
            own_release: (val) => {{
                const count = (ownershipStorage.get(val) || 1) - 1;
                if (count <= 0) {{
                    ownershipStorage.delete(val);
                }} else {{
                    ownershipStorage.set(val, count);
                }}
            }},
        }},
        env: {{
            now: () => BigInt(Date.now()),
        }},
        webgpu: {{
            requestAdapter: async () => globalThis.navigator?.gpu?.requestAdapter(),
            requestDevice: async (adapter) => adapter?.requestDevice(),
        }},
        webgl: {{
            getContext: (canvasId) => globalThis.document?.getElementById(canvasId)?.getContext('webgl2'),
        }},
        "datara:ui": {{
            create_element: (tagPtr) => {{
                if (typeof document === 'undefined') return 0n;
                const tag = readString(tagPtr);
                const el = document.createElement(tag);
                const handle = nextDomHandle++;
                domStorage.set(handle, el);
                return handle;
            }},
            set_text: (elHandle, textPtr) => {{
                const el = domStorage.get(elHandle);
                if (el) el.textContent = readString(textPtr);
                return elHandle;
            }},
            set_attribute: (elHandle, namePtr, valPtr) => {{
                const el = domStorage.get(elHandle);
                if (el) el.setAttribute(readString(namePtr), readString(valPtr));
                return elHandle;
            }},
            append_child: (parentHandle, childHandle) => {{
                if (typeof document === 'undefined') return parentHandle;
                const parent = parentHandle === 0n ? (document.getElementById('app') || document.body) : domStorage.get(parentHandle);
                const child = domStorage.get(childHandle);
                if (parent && child) parent.appendChild(child);
                return parentHandle;
            }},
            mount_root: (rootHandle) => {{
                if (typeof document === 'undefined') return rootHandle;
                const root = domStorage.get(rootHandle);
                const app = document.getElementById('app') || document.body;
                if (root && app) app.appendChild(root);
                return rootHandle;
            }},
        }}
    }};

    // Compositional capability imports (only included if granted and used)
    {fs_binding}
    {net_binding}
    {sys_binding}

    // Merge with any caller overrides
    for (const [k, v] of Object.entries(customImports)) {{
        importObject[k] = Object.assign(importObject[k] || {{}}, v);
    }}

    const {{ instance }} = await WebAssembly.instantiate(wasmBytes, importObject);
    memoryInstance = instance.exports.memory;
    return instance.exports;
}}
"#,
            module_name = module_name,
            fs_binding = if has_fs {
                r#"importObject["datara:fs@1.0"] = {
        read: async (pathPtr) => {
            const p = readString(pathPtr);
            if (typeof process !== 'undefined' && process.versions?.node) {
                const fs = await import('fs');
                try { return BigInt(fs.readFileSync(p).length); } catch (e) { return 0n; }
            }
            return 0n;
        },
        write: (pathPtr, contentPtr) => {
            const p = readString(pathPtr);
            return 1n;
        }
    };"#
            } else {
                "// datara:fs@1.0: NOT GRANTED (physically omitted from imports)"
            },
            net_binding = if has_net {
                r#"importObject["datara:net@1.0"] = {
        connect: (hostPtr, port) => 1n,
        listen: (port, backlog) => 1n,
        http_get: (urlPtr) => 200n,
    };"#
            } else {
                "// datara:net@1.0: NOT GRANTED (physically omitted from imports)"
            },
            sys_binding = if has_sys {
                r#"importObject["datara:sys@1.0"] = {
        exec: (cmdPtr) => 0n,
        env: (keyPtr) => 0n,
        env_get: (keyPtr) => 0n,
        clock_now: () => BigInt(Date.now()),
    };"#
            } else {
                "// datara:sys@1.0: NOT GRANTED (physically omitted from imports)"
            }
        )
    }
}
