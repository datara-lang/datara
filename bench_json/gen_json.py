#!/usr/bin/env python3
"""Generates the bench_json 10 MB workload (deterministic, ASCII only).

Format: a top-level array of flat records
    {"id": K, "name": "user_NNNNNNN", "score": F, "active": B, "tags": ["tK", "xK"]}

Records are ~100 bytes; the record count is chosen so the file is ~10 MB.
Usage: python gen_json.py [out_path] [target_mb]
"""
import json
import sys


def main() -> None:
    out_path = sys.argv[1] if len(sys.argv) > 1 else "data_10mb.json"
    target_mb = float(sys.argv[2]) if len(sys.argv) > 2 else 10.0

    # Estimate record count: one record with a 9-char name is ~100 bytes.
    n = int(target_mb * 1024 * 1024 / 100)

    parts = []
    total = 0
    k = 0
    while total < target_mb * 1024 * 1024:
        rec = {
            "id": k,
            "name": f"user_{k:08d}",
            "score": (k % 1000) / 8.0,
            "active": (k % 2 == 0),
            "tags": [f"t{k % 17}", f"x{k % 6}"],
        }
        s = json.dumps(rec, separators=(",", ":"))
        parts.append(s)
        total += len(s) + 1
        k += 1

    doc = "[" + ",".join(parts) + "]"
    with open(out_path, "w", encoding="utf-8", newline="") as f:
        f.write(doc)

    size = len(doc.encode("utf-8"))
    print(f"records={k} bytes={size} ({size / 1024 / 1024:.2f} MB) -> {out_path}")


if __name__ == "__main__":
    main()
