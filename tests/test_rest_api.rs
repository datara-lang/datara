//! PLAN_V155 B2: the rest_api flagship contract — full CRUD over fibers
//! with stdlib.http.client + stdlib.text.regex dogfooding (AOT tier).
//!
//! Every route of examples/rest_api/main.dtr is pinned here in one
//! self-contained program: health, create (regex extracts "name" from
//! the POST body), list, get-by-id, and the 404 path.

use forgen::driver::ForgenCompiler;

#[test]
fn rest_api_crud_round_trip() {
    let src = r#"
use stdlib.net.net
use stdlib.http.client
use stdlib.text.regex

mut s_ids: List<Int> = []
mut s_names: List<Str> = []
mut s_next_id: Int = 0

fn store_add(name: Str) -> Int {
    s_next_id = s_next_id + 1
    s_ids = s_ids.push(s_next_id)
    s_names = s_names.push(name)
    return s_next_id
}

fn item_json(id: Int, name: Str) -> Str {
    return "{\"id\":" + int_to_str(id) + ",\"name\":\"" + name + "\"}"
}

fn store_find_name_by_id(id: Int) -> Str {
    mut i = 0
    while i < s_ids.len() {
        if s_ids.get(i) == id {
            return s_names.get(i)
        }
        i = i + 1
    }
    return ""
}

fn store_list_json() -> Str {
    mut acc = "["
    mut i = 0
    while i < s_ids.len() {
        if i > 0 {
            acc = acc + ","
        }
        acc = acc + item_json(s_ids.get(i), s_names.get(i))
        i = i + 1
    }
    return acc + "]"
}

fn extract_name(body: Str) -> Str {
    let p = "\"name\"\s*:\s*\"([^\"]+)\""
    val gs = regex_find_groups(p, body)
    if gs.is_success && gs.value.len() >= 2 {
        return gs.value.get(1)
    }
    return ""
}

fn respond(peer: Int, status: Int, body: Str) {
    let status_txt = if status == 201 {
        "201 Created"
    } else if status == 404 {
        "404 Not Found"
    } else {
        "200 OK"
    }
    let head = "HTTP/1.1 " + status_txt + "\r\nContent-Type: application/json\r\nContent-Length: " + int_to_str(str_len(body)) + "\r\nConnection: close\r\n\r\n"
    tcp_send(peer, head + body)
}

fn route(method: Str, path: Str, body: Str) -> Str {
    // returns "STATUS<TAB>BODY"
    if method == "GET" && path == "/health" {
        return "200\t{\"ok\":true}"
    }
    if method == "GET" && path == "/api/items" {
        return "200\t" + store_list_json()
    }
    if method == "POST" && path == "/api/items" {
        let name = extract_name(body)
        if str_len(name) == 0 {
            return "400\t{\"error\":\"missing name\"}"
        }
        let id = store_add(name)
        return "201\t" + item_json(id, name)
    }
    let single = "/api/items/"
    if method == "GET" && str_starts_with(path, single) {
        let id_str = str_substring(path, str_len(single), str_len(path) - str_len(single))
        let id = str_to_int(id_str)
        let name = store_find_name_by_id(id)
        if str_len(name) > 0 {
            return "200\t" + item_json(id, name)
        }
        return "404\t{\"error\":\"not found\"}"
    }
    return "404\t{\"error\":\"no route\"}"
}

fn handle_client(peer: Int) {
    let raw = tcp_recv(peer, 8192, 5000)
    if raw.is_success {
        let line_end = str_index_of(raw.value, "\r\n")
        mut line = raw.value
        if line_end > 0 {
            line = str_substring(raw.value, 0, line_end)
        }
        let sp1 = str_index_of(line, " ")
        let rest = str_substring(line, sp1 + 1, str_len(line) - sp1 - 1)
        let sp2 = str_index_of(rest, " ")
        let method = str_substring(line, 0, sp1)
        let path = str_substring(rest, 0, sp2)
        let body_idx = str_index_of(raw.value, "\r\n\r\n")
        let body = if body_idx > 0 {
            str_substring(raw.value, body_idx + 4, str_len(raw.value) - body_idx - 4)
        } else {
            ""
        }
        val result = route(method, path, body)
        let tidx = str_index_of(result, "\t")
        let status = str_to_int(str_substring(result, 0, tidx))
        let resp_body = str_substring(result, tidx + 1, str_len(result) - tidx - 1)
        respond(peer, status, resp_body)
    }
    tcp_close(peer)
}

fn serve(count: Int) {
    val bound = tcp_listen(45355)
    if bound.is_success {
        mut served = 0
        while served < count {
            val acc = tcp_accept(bound.value)
            if acc.is_success {
                served = served + 1
                handle_client(acc.value)
            }
        }
        tcp_close(bound.value)
    }
}

fn client_work() {
    fiber_sleep_ms(150)
    let client = HttpClient.new()
    let h = client.get("127.0.0.1", 45355, "/health")
    if h.is_success {
        out fmt"health {h.value.status} {h.value.body}"
    }
    let c1 = client.post("127.0.0.1", 45355, "/api/items", "{\"name\":\"alpha\"}", "application/json")
    if c1.is_success {
        out fmt"create {c1.value.status} {c1.value.body}"
    }
    let lst = client.get("127.0.0.1", 45355, "/api/items")
    if lst.is_success {
        out fmt"list {lst.value.status} {lst.value.body}"
    }
    let one = client.get("127.0.0.1", 45355, "/api/items/1")
    if one.is_success {
        out fmt"get {one.value.status} {one.value.body}"
    }
    let missing = client.get("127.0.0.1", 45355, "/api/items/99")
    if missing.is_success {
        out fmt"miss {missing.value.status} {missing.value.body}"
    }
}

fn main() {
    unsafe(justification: "REST API round trip test") {
        fiber_spawn(serve, 5)
        fiber_spawn(client_work, 0)
        fiber_run_until_done()
        out "REST-TEST-DONE"
    }
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(src, "rest_api_test.dtr", None);
    assert!(
        res.success,
        "compilation must succeed: {:?}\n{}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("must produce a native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    if code != 0 {
        eprintln!("stderr: {stderr}");
    }
    assert_eq!(code, 0, "no crash: the CRUD round trip must complete");
    assert_eq!(
        stdout.trim(),
        "health 200 {\"ok\":true}\ncreate 201 {\"id\":1,\"name\":\"alpha\"}\nlist 200 [{\"id\":1,\"name\":\"alpha\"}]\nget 200 {\"id\":1,\"name\":\"alpha\"}\nmiss 404 {\"error\":\"not found\"}\nREST-TEST-DONE"
    );
}
