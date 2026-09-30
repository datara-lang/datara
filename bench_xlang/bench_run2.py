import subprocess, time, statistics, os

def bench(args, n=15):
    # warmup 2 runs
    for _ in range(2):
        subprocess.run(args, capture_output=True)
    ts = []
    for _ in range(n):
        t0 = time.perf_counter()
        r = subprocess.run(args, capture_output=True)
        ts.append((time.perf_counter() - t0) * 1000)
    return statistics.median(ts), min(ts), max(ts), r.stdout.decode().strip()

def kb(p):
    return os.path.getsize(p) / 1024.0

out = []
out.append("=== Cross-language benchmark, POST-ovf-fix (Windows 11 x64, warm cache, n=15, wall-time) ===")
out.append("Datara: forgen build --llvm --opt=speed (LLVM AOT backend). Contract: print result.")
out.append("")

def run_task(title, rows):
    out.append(title)
    res = {}
    for name, exe in rows:
        m, lo, hi, stdout = bench([exe])
        res[name] = (m, lo, hi, kb(exe), stdout)
    fastest = min(v[0] for v in res.values())
    for name, (m, lo, hi, size, stdout) in res.items():
        out.append(f"  {name:28} median {m:8.2f} ms  min {lo:7.2f}  max {hi:7.2f}  bin {size:6.1f} KB  x{m/fastest:5.2f}  out={stdout}")
    out.append("")

run_task("100M iterations, acc += i & 255 (expect 12750000000)", [
    ("Datara LLVM speed", "./loops_dt_speed.exe"),
    ("Datara LLVM tiny",  "./loops_dt_tiny.exe"),
    ("C gcc -O2",         "./loops_c_o2.exe"),
    ("C gcc -O3",         "./loops_c_o3.exe"),
    ("Rust rustc -O",     "./loops_rs.exe"),
])

run_task("fib(30) x10 (expect 8320400)", [
    ("Datara LLVM speed", "./fib_dt_speed.exe"),
    ("C gcc -O2",         "./fib_c.exe"),
    ("Rust rustc -O",     "./fib_rs.exe"),
])

m, lo, hi, _ = bench(["python", "loops_full.py"], n=3)
out.append(f"  Python 3.14 100M loop: median {m:9.2f} ms")
m, lo, hi, _ = bench(["python", "fib.py"], n=5)
out.append(f"  Python 3.14 fib x10:   median {m:9.2f} ms")

with open("RESULTS_POST_FIX.txt", "w", encoding="utf-8") as f:
    f.write("\n".join(out))
print("\n".join(out))
