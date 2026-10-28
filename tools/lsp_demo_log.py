#!/usr/bin/env python3
"""M3 demo: drive `forgen lsp` over stdio and record hover/goto-def/
references/rename request-response logs (PLAN_V150 M3 acceptance).

Usage: python tools/lsp_demo_log.py [path-to-forgen]
Writes docs/lsp_demo_log.txt.
"""

import json
import subprocess
import sys
import threading
import time

FORGEN = sys.argv[1] if len(sys.argv) > 1 else "target/release/forgen.exe"
OUT = "docs/lsp_demo_log.txt"

DEMO = """fn helper(a: Int) -> Int {
    return a * 2
}

fn main() {
    val n = helper(21)
    out n
}
"""

URI = "file:///demo_lsp.dtr"


def frame(msg: dict) -> bytes:
    body = json.dumps(msg).encode("utf-8")
    return b"Content-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body


class FramedReader:
    def __init__(self, stream):
        self.stream = stream
        self.buf = b""
        self.messages = []
        self.lock = threading.Lock()

    def _loop(self):
        while True:
            chunk = self.stream.read(1)
            if not chunk:
                break
            with self.lock:
                self.buf += chunk
                while b"\r\n\r\n" in self.buf:
                    head, rest = self.buf.split(b"\r\n\r\n", 1)
                    length = 0
                    for line in head.split(b"\r\n"):
                        if line.lower().startswith(b"content-length:"):
                            length = int(line.split(b":", 1)[1].strip())
                    if len(rest) < length:
                        break
                    body, self.buf = rest[:length], rest[length:]
                    self.messages.append(json.loads(body.decode("utf-8")))

    def wait_response(self, rid: int, timeout: float = 15.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            with self.lock:
                for m in self.messages:
                    if m.get("id") == rid:
                        return m
            time.sleep(0.05)
        return None


def main() -> None:
    proc = subprocess.Popen(
        [FORGEN, "lsp"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    reader = FramedReader(proc.stdout)
    threading.Thread(target=reader._loop, daemon=True).start()

    def send(msg):
        proc.stdin.write(frame(msg))
        proc.stdin.flush()

    lines = []

    send({"jsonrpc": "2.0", "id": 1, "method": "initialize",
          "params": {"capabilities": {}}})
    reader.wait_response(1)
    send({"jsonrpc": "2.0", "method": "initialized", "params": {}})
    send({"jsonrpc": "2.0", "method": "textDocument/didOpen",
          "params": {"textDocument": {"uri": URI, "languageId": "datara",
                                       "version": 1, "text": DEMO}}})
    time.sleep(0.5)

    requests = [
        ("hover", 2, "textDocument/hover",
         {"line": 5, "character": 13}, None),
        ("goto-definition", 3, "textDocument/definition",
         {"line": 5, "character": 13}, None),
        ("references", 4, "textDocument/references",
         {"line": 0, "character": 3},
         {"context": {"includeDeclaration": True}}),
        ("rename", 5, "textDocument/rename",
         {"line": 0, "character": 3},
         {"newName": "double_value"}),
    ]
    for name, rid, method, pos, extra in requests:
        params = {"textDocument": {"uri": URI},
                  "position": {"line": pos["line"],
                                "character": pos["character"]}}
        if extra:
            params.update(extra)
        msg = {"jsonrpc": "2.0", "id": rid, "method": method, "params": params}
        send(msg)
        resp = reader.wait_response(rid)
        lines.append(f"=== {name} ===")
        lines.append("request : " + json.dumps(msg))
        lines.append("response: " + json.dumps(resp))
        lines.append("")

    proc.stdin.close()
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()

    with open(OUT, "w", encoding="utf-8") as f:
        f.write("# forgen lsp demo log (M3): hover / goto-definition / "
                "references / rename\n")
        f.write("# demo source: fn helper(a: Int) -> Int { return a * 2 } "
                "+ main calling helper(21)\n\n")
        f.write("\n".join(lines))
    print(f"wrote {OUT} ({len(lines)} lines)")


if __name__ == "__main__":
    main()
