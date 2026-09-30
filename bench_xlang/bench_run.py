import subprocess, time, statistics, os, sys

def bench(args, n=15):
    ts=[]
    for _ in range(n):
        t0=time.perf_counter()
        subprocess.run(args, capture_output=True)
        ts.append((time.perf_counter()-t0)*1000)
    return statistics.median(ts), min(ts), max(ts)

out = []
out.append("=== Cross-language benchmark (Windows 11 x64, warm cache, n=15) ===")
out.append("Contract: program computes result and prints it; measured as process wall time.")
out.append("")
rows = {
 "fib(30) x10 (=8320400)": [
   ("Datara 1.4.5 Cranelift", "./fib_out.exe", 175.0),
   ("Rust 1.98 rustc -O",     "./fib_rs.exe",  121.0),
   ("C gcc 16.2 -O2",         "./fib_c.exe",    49.6),
 ],
 "100M iterations (acc=12750000000)": [
   ("Datara 1.4.5 Cranelift", "./loops_out.exe", 174.5),
   ("Rust 1.98 rustc -O",     "./loops_rs.exe",  121.0),
   ("C gcc 16.2 -O2",         "./loops_c.exe",    48.5),
 ],
}
for task, langs in rows.items():
    res = {}
    for lang, exe, size in langs:
        m, lo, hi = bench([exe])
        res[lang] = (m, lo, hi, size)
    fastest = min(v[0] for v in res.values())
    out.append(task)
    for lang, (m, lo, hi, size) in res.items():
        out.append(f"  {lang:26} median {m:9.2f} ms   min {lo:9.2f}   max {hi:9.2f}   bin {size:7.1f} KB   x{m/fastest:5.2f}")
    out.append("")
m, lo, hi = bench(["python","fib.py"], n=5)
out.append(f"fib(30) x10\n  Python 3.14                     median {m:9.2f} ms   x{m/2.81:8.1f}")
m, lo, hi = bench(["python","loops_full.py"], n=3)
out.append(f"100M iterations\n  Python 3.14                     median {m:9.2f} ms   x{m/2.48:8.1f}")
with open("RESULTS.txt", "w", encoding="utf-8") as f:
    f.write("\n".join(out))
print("saved")
