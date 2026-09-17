import ctypes, json, os, sys, urllib.request, urllib.error
from ctypes import wintypes

class CREDENTIAL(ctypes.Structure):
    _fields_ = [
        ('Flags', wintypes.DWORD),
        ('Type', wintypes.DWORD),
        ('TargetName', wintypes.LPWSTR),
        ('Comment', wintypes.LPWSTR),
        ('LastWritten', wintypes.FILETIME),
        ('CredentialBlobSize', wintypes.DWORD),
        ('CredentialBlob', ctypes.c_void_p),
        ('Persist', wintypes.DWORD),
        ('AttributeCount', wintypes.DWORD),
        ('Attributes', ctypes.c_void_p),
        ('TargetAlias', wintypes.LPWSTR),
        ('UserName', wintypes.LPWSTR),
    ]

def get_token():
    pcred = ctypes.POINTER(CREDENTIAL)()
    ok = ctypes.windll.advapi32.CredReadW('git:https://waters1ze@github.com', 1, 0, ctypes.byref(pcred))
    if not ok:
        raise RuntimeError('Failed to read git credential')
    raw = ctypes.string_at(pcred.contents.CredentialBlob, pcred.contents.CredentialBlobSize)
    ctypes.windll.advapi32.CredFree(pcred)
    try:
        token = raw.decode('utf-16').strip()
        if token.startswith('ghp_') or token.startswith('github_pat_'):
            return token
    except Exception:
        pass
    return raw.decode('utf-8', errors='ignore').strip()

def api(token, method, url, data=None, content_type='application/vnd.github+json'):
    headers = {
        'Authorization': f'Bearer {token}',
        'User-Agent': 'Datara-Release-Agent',
        'Accept': 'application/vnd.github+json',
    }
    if content_type:
        headers['Content-Type'] = content_type
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    with urllib.request.urlopen(req) as resp:
        if resp.status == 204:
            return None
        return json.loads(resp.read().decode('utf-8'))

def main():
    token = get_token()
    repo = 'datara-lang/datara'
    tag = 'v1.4.2'
    title = 'v1.4.2 - Bridge Hub, Capabilities 2.0, DPM Registry, Socket & Fast Input Ergonomics'

    with open('dist/SHA256SUMS.txt', 'r', encoding='utf-8') as f:
        sums_text = f.read().strip()

    body = (
        "## [1.4.2] - 2026-09-17\n\n"
        "### Added\n"
        "- **Declarative Foreign Bridges (Bridge Hub)**: Added declarative `bridge <lang>::<module> { fn ... }` syntax across Python, JavaScript, C, and Rust with strict type whitelist enforcement (`E-BRIDGE-002`), call-site argument type checking (`E-BRIDGE-001`), and supported language validation (`E-BRIDGE-003`).\n"
        "- **Capabilities 2.0 Security Model**: Added manifest `[capabilities]` section supporting `fs-read`, `fs-write`, `net-listen`, `net-connect`, `env`, and `exec` with glob matching; allowed targets bypass `unsafe`, while missing permissions or glob violations trigger `E-CAP-001` and `E-CAP-002` with mandatory `unsafe(justification: \"...\")` escalation for `exec` and sockets.\n"
        "- **DPM Bridge Package Registry**: Added standard bridge package format (`bridge.toml` + `bridge.dtr`), CLI commands (`dpm add`, `dpm list`, `dpm remove`, `dpm search`, `dpm publish --dry-run`), and automatic `./dpm_packages/*/bridge.dtr` compiler scanning.\n"
        "- **Socket Ergonomics (Timeouts & Nonblocking)**: Added `socket_set_timeout(sock, ms) -> Outcome<Unit>`, `socket_nonblocking(sock, on: Bool) -> Outcome<Unit>`, and `socket_recv_outcome(sock, max_bytes) -> Outcome<Str>` returning `Outcome.err(\"would-block\")` on nonblocking read without thread stalling.\n"
        "- **Fast Safe User Input Ergonomics**: Added zero-allocation fast scanners `read_int() -> Int`, `input_int(prompt) -> Int`, `read_float() -> Float`, `input_float(prompt) -> Float`, and dynamically resizing `read_line() -> Str` and `input(prompt) -> Str` handling arbitrarily long inputs safely without buffer overflows.\n"
        "- **Python Bridge Optimization & Doctor Diagnostics**: Enforced thread-safe single `Py_Initialize` per process, added `py_eval_batch(json_exprs)` for single GIL acquisition across multiple evaluations, and updated `forgen doctor --bridges` to report overhead categories (`[category: C-ABI (ns)]` vs `[category: in-process (µs)]`).\n\n"
        "### Checksums (SHA-256)\n"
        "```text\n"
        + sums_text + "\n"
        "```\n"
    )

    release = api(token, 'GET', f'https://api.github.com/repos/{repo}/releases/tags/{tag}')
    rel_id = release['id']
    print('Updating release body and title...')
    api(token, 'PATCH', f'https://api.github.com/repos/{repo}/releases/{rel_id}', data=json.dumps({'body': body, 'name': title}).encode('utf-8'), content_type='application/json')

    upload_base = release['upload_url'].split('{')[0]
    for asset in release.get('assets', []):
        print(f'Deleting old asset: {asset["name"]} (id={asset["id"]})...')
        api(token, 'DELETE', f'https://api.github.com/repos/{repo}/releases/assets/{asset["id"]}')

    files_to_upload = [
        'Datara-Setup.exe',
        'Datara-v1.4.2-Setup.exe',
        'forgen-windows-x64.zip',
        'forgen-v1.4.2-windows-x64.zip',
        'forgen-linux-x64.zip',
        'forgen-v1.4.2-linux-x64.zip',
        'forgen-darwin-arm64.zip',
        'forgen-v1.4.2-darwin-arm64.zip',
        'SHA256SUMS.txt',
    ]

    for fname in files_to_upload:
        fpath = os.path.join('dist', fname)
        if not os.path.exists(fpath):
            continue
        print(f'Uploading fresh {fname} ({os.path.getsize(fpath)} bytes)...')
        with open(fpath, 'rb') as fp:
            data = fp.read()
        ct = 'application/octet-stream'
        if fname.endswith('.txt'): ct = 'text/plain'
        elif fname.endswith('.zip'): ct = 'application/zip'
        elif fname.endswith('.exe'): ct = 'application/vnd.microsoft.portable-executable'
        up_url = f'{upload_base}?name={fname}'
        req = urllib.request.Request(up_url, data=data, headers={
            'Authorization': f'Bearer {token}',
            'User-Agent': 'Datara-Release-Agent',
            'Accept': 'application/vnd.github+json',
            'Content-Type': ct,
            'Content-Length': str(len(data))
        }, method='POST')
        with urllib.request.urlopen(req) as resp:
            print(f'  [OK] Uploaded {fname}')

    print('\nRelease v1.4.2 synchronized successfully!')

if __name__ == '__main__':
    main()
