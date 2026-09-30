//! M3 LSP production tests (PLAN_V150): hover, goto definition, find
//! references and rename on a fixed demo project source. All requests go
//! through LspServer::handle_request with JSON-RPC payloads, mirroring the
//! real editor protocol.

use forgen::lsp::LspServer;
use serde_json::Value;

const DEMO: &str = "fn helper(a: Int) -> Int {\n    return a * 2\n}\n\nfn main() {\n    let n = helper(21)\n    out n\n}\n";

fn open_request(uri: &str, text: &str) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "textDocument/didOpen",
        "params": { "textDocument": { "uri": uri, "languageId": "datara", "version": 1, "text": text } }
    })
}

fn run(server: &LspServer, req: Value) -> Value {
    let payload = serde_json::to_string(&req).unwrap();
    let mut buf: Vec<u8> = Vec::new();
    server
        .handle_request(&serde_json::from_str(&payload).unwrap(), &mut buf)
        .unwrap();
    let output = String::from_utf8(buf).unwrap();
    let start = output.find('{').expect("JSON body start");
    serde_json::from_str(&output[start..]).expect("valid JSON")
}

fn result_of(resp: &Value) -> &Value {
    resp.get("result").expect("response must carry a result")
}

#[test]
fn lsp_references_finds_all_occurrences() {
    let server = LspServer::new();
    let uri = "file:///demo.dtr";
    server
        .handle_request(
            &serde_json::from_str(&serde_json::to_string(&open_request(uri, DEMO)).unwrap())
                .unwrap(),
            &mut Vec::new(),
        )
        .unwrap();

    // `helper` appears on line 0 (definition) and line 5 (call).
    let resp = run(
        &server,
        serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/references",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 5, "character": 13 },
                "context": { "includeDeclaration": true }
            }
        }),
    );
    let refs = result_of(&resp).as_array().expect("references array");
    assert_eq!(
        refs.len(),
        2,
        "helper must have definition + call: {refs:?}"
    );
    assert_eq!(refs[0]["range"]["start"]["line"], 0);
    assert_eq!(refs[1]["range"]["start"]["line"], 5);
}

#[test]
fn lsp_rename_returns_workspace_edit() {
    let server = LspServer::new();
    let uri = "file:///demo.dtr";
    server
        .handle_request(
            &serde_json::from_str(&serde_json::to_string(&open_request(uri, DEMO)).unwrap())
                .unwrap(),
            &mut Vec::new(),
        )
        .unwrap();

    let resp = run(
        &server,
        serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "textDocument/rename",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 0, "character": 3 },
                "newName": "double_value"
            }
        }),
    );
    let result = result_of(&resp);
    let edits = result["changes"][uri]
        .as_array()
        .expect("workspace edit for the demo uri");
    assert_eq!(edits.len(), 2, "both occurrences renamed");
    assert!(edits.iter().all(|e| e["newText"] == "double_value"));

    // Apply the edit to the source text and verify the program is the
    // renamed one (same shape, new identifier everywhere).
    let mut renamed_lines: Vec<String> = DEMO.lines().map(|s| s.to_string()).collect();
    for e in edits {
        let line = e["range"]["start"]["line"].as_u64().unwrap() as usize;
        let sc = e["range"]["start"]["character"].as_u64().unwrap() as usize;
        let ec = e["range"]["end"]["character"].as_u64().unwrap() as usize;
        let l = renamed_lines[line].clone();
        renamed_lines[line] = format!("{}{}{}", &l[..sc], "double_value", &l[ec..]);
    }
    let renamed = renamed_lines.join("\n");
    assert!(renamed.contains("fn double_value(a: Int)"));
    assert!(renamed.contains("= double_value(21)"));

    // The renamed source must still compile: rename is semantics-preserving.
    let compiler = forgen::driver::ForgenCompiler::new("release");
    let res = compiler.compile_source_native(&renamed, "lsp_rename_check.dtr", None);
    assert!(res.success, "renamed source must compile: {:?}", res.error);
    let _ = res.exe_path.map(std::fs::remove_file);
}

#[test]
fn lsp_definition_and_hover_work() {
    let server = LspServer::new();
    let uri = "file:///demo.dtr";
    server
        .handle_request(
            &serde_json::from_str(&serde_json::to_string(&open_request(uri, DEMO)).unwrap())
                .unwrap(),
            &mut Vec::new(),
        )
        .unwrap();

    // goto definition from the call site (line 5) jumps to line 0.
    let resp = run(
        &server,
        serde_json::json!({
            "jsonrpc": "2.0", "id": 4, "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 5, "character": 13 }
            }
        }),
    );
    let def = result_of(&resp);
    assert_eq!(
        def["range"]["start"]["line"], 0,
        "definition jumps to fn helper"
    );

    // hover reports the symbol.
    let resp = run(
        &server,
        serde_json::json!({
            "jsonrpc": "2.0", "id": 5, "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 0, "character": 3 }
            }
        }),
    );
    assert!(result_of(&resp)["contents"].is_object() || result_of(&resp)["contents"].is_string());
}
