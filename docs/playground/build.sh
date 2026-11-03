#!/bin/bash
# Build all playground WASM demos: ./build.sh
cd "$(dirname "$0")"
for demo in fib sieve matmul; do
    echo "Building $demo..."
    ../../target/release/forgen.exe build --target wasm ${demo}.dtr 2>&1
done
echo "Done. Open index.html in a browser."
