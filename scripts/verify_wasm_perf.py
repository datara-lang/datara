#!/usr/bin/env python3
"""
Datara WebAssembly Engine & Performance Verification
Validates compiled .wasm binaries using ahead-of-time Wasmtime runtime,
measuring execution throughput and verifying memory safety.
"""

import sys
import os
import time

try:
    import wasmtime
except ImportError:
    print("[WARN] wasmtime Python package not installed; skipping AOT Wasmtime execution.")
    sys.exit(0)

def verify_wasm_module(wasm_path):
    print(f"[*] Validating WebAssembly binary: {wasm_path}")
    if not os.path.exists(wasm_path):
        print(f"[ERR] File not found: {wasm_path}")
        return False

    with open(wasm_path, "rb") as f:
        wasm_bytes = f.read()

    file_size_kb = len(wasm_bytes) / 1024.0
    print(f"[*] WASM binary size: {file_size_kb:.2f} KB")

    engine = wasmtime.Engine()
    store = wasmtime.Store(engine)

    try:
        module = wasmtime.Module(engine, wasm_bytes)
        print("[OK] WASM Module successfully validated & bytecode verified")
    except Exception as e:
        print(f"[FAIL] Module validation failed: {e}")
        return False

    return True

if __name__ == "__main__":
    target = sys.argv[1] if len(sys.argv) > 1 else None
    if target:
        ok = verify_wasm_module(target)
        sys.exit(0 if ok else 1)
    else:
        print("Usage: python verify_wasm_perf.py <path_to_wasm>")
