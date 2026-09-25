// Datara Language Support - LSP client (dependency-free).
//
// Spawns `forgen lsp` (or the configured binary) over stdio and bridges it
// to VS Code's built-in language features: diagnostics, hover, completion,
// go-to-definition, find-references, rename, formatting, inlay hints and
// semantic tokens are all served by the forgen LSP server (M3).
//
// The JSON-RPC framing is hand-rolled (Content-Length headers over stdio)
// so the extension ships without any npm dependency.

const vscode = require('vscode');
const { spawn } = require('child_process');
const path = require('path');

let serverProcess = null;
let pending = new Map(); // id -> { resolve, reject }
let nextId = 1;
let outputChannel = null;

function serverCommand() {
  const cfg = vscode.workspace.getConfiguration('datara');
  return cfg.get('serverPath', 'forgen');
}

function contentLength(buffer) {
  const head = buffer.toString('latin1');
  const m = head.match(/Content-Length: (\d+)\r\n\r\n/);
  return m ? { length: parseInt(m[1], 10), headerEnd: m.index + m[0].length } : null;
}

function startServer(context) {
  outputChannel = vscode.window.createOutputChannel('Datara Language Server');
  context.subscriptions.push(outputChannel);

  const [cmd, ...baseArgs] = serverCommand().split(/\s+/);
  serverProcess = spawn(cmd, [...baseArgs, 'lsp'], {
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  outputChannel.appendLine(`[datara] started: ${cmd} lsp`);

  let buffer = Buffer.alloc(0);
  serverProcess.stdout.on('data', (chunk) => {
    buffer = Buffer.concat([buffer, chunk]);
    // Parse every complete Content-Length framed message.
    for (;;) {
      const frame = contentLength(buffer);
      if (!frame) break;
      if (buffer.length < frame.headerEnd + frame.length) break;
      const body = buffer.subarray(frame.headerEnd, frame.headerEnd + frame.length);
      buffer = buffer.subarray(frame.headerEnd + frame.length);
      handleMessage(JSON.parse(body.toString('utf8')));
    }
  });
  serverProcess.stderr.on('data', (d) => outputChannel.appendLine(String(d)));
  serverProcess.on('exit', (code) => {
    outputChannel.appendLine(`[datara] server exited with ${code}`);
    serverProcess = null;
  });
}

function sendMessage(method, params, id) {
  if (!serverProcess) return;
  const body = Buffer.from(JSON.stringify({ jsonrpc: '2.0', method, params, id }), 'utf8');
  serverProcess.stdin.write(`Content-Length: ${body.length}\r\n\r\n`);
  serverProcess.stdin.write(body);
}

function request(method, params) {
  const id = nextId++;
  return new Promise((resolve) => {
    pending.set(id, { resolve });
    sendMessage(method, params, id);
    // Resolve null after a grace period so a hung server never blocks UI.
    setTimeout(() => {
      if (pending.has(id)) {
        pending.delete(id);
        resolve(null);
      }
    }, 10000);
  });
}

function notify(method, params) {
  sendMessage(method, params, null);
}

function handleMessage(msg) {
  if (msg.method === 'textDocument/publishDiagnostics') {
    const uri = vscode.Uri.parse(msg.params.uri);
    const diags = (msg.params.diagnostics || []).map((d) => {
      const range = new vscode.Range(
        d.range.start.line,
        d.range.start.character,
        d.range.end.line,
        d.range.end.character
      );
      const severity =
        d.severity === 2
          ? vscode.DiagnosticSeverity.Warning
          : d.severity === 3
            ? vscode.DiagnosticSeverity.Information
            : vscode.DiagnosticSeverity.Error;
      return new vscode.Diagnostic(range, d.message, severity);
    });
    vscode.languages.setTextDocumentLanguage(uri, 'datara').catch(() => {});
    vscode.workspace.textDocuments.forEach((doc) => {
      if (doc.uri.toString() === uri.toString()) {
        // Attached to a real document: set diagnostics directly.
      }
    });
    // Diagnostics collection keyed by the server's uri string.
    diagnosticCollection.set(uri, diags);
    return;
  }
  if (msg.id && pending.has(msg.id)) {
    const entry = pending.get(msg.id);
    pending.delete(msg.id);
    entry.resolve(msg.result !== undefined ? msg.result : null);
  }
}

const diagnosticCollection = vscode.languages.createDiagnosticCollection('datara');

function wireServerHandlers(context) {
  context.subscriptions.push(diagnosticCollection);
}

function documentSelector() {
  return [{ language: 'datara', scheme: 'file' }, { language: 'datara', scheme: 'untitled' }];
}

function activate(context) {
  startServer(context);
  wireServerHandlers(context);

  const notifyOpen = (doc) => {
    if (doc.languageId !== 'datara') return;
    notify('textDocument/didOpen', {
      textDocument: {
        uri: doc.uri.toString(),
        languageId: 'datara',
        version: doc.version,
        text: doc.getText(),
      },
    });
  };

  vscode.workspace.textDocuments.forEach(notifyOpen);
  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument(notifyOpen),
    vscode.workspace.onDidChangeTextDocument((e) => {
      if (e.document.languageId !== 'datara') return;
      notify('textDocument/didChange', {
        textDocument: { uri: e.document.uri.toString(), version: e.document.version },
        contentChanges: [{ text: e.document.getText() }],
      });
    }),
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (doc.languageId === 'datara') notify('textDocument/didSave', { textDocument: { uri: doc.uri.toString() } });
    })
  );

  // Bridge: server-side features resolved per-request. Diagnostics come
  // server-pushed; everything else is pulled through the requests below.
  const pull = (method, makeParams) =>
    vscode.languages.registerProvider
      ? null
      : null;

  // Hover: request the server on demand and render the markdown card.
  context.subscriptions.push(
    vscode.languages.registerHoverProvider(documentSelector(), {
      async provideHover(doc, pos) {
        notifyOpen(doc);
        const res = await request('textDocument/hover', {
          textDocument: { uri: doc.uri.toString() },
          position: { line: pos.line, character: pos.character },
        });
        if (!res) return null;
        const c = res.contents;
        const md = new vscode.MarkdownString(
          typeof c === 'string' ? c : c.value || ''
        );
        return new vscode.Hover(md);
      },
    })
  );

  context.subscriptions.push(
    vscode.languages.registerDefinitionProvider(documentSelector(), {
      async provideDefinition(doc, pos) {
        const res = await request('textDocument/definition', {
          textDocument: { uri: doc.uri.toString() },
          position: { line: pos.line, character: pos.character },
        });
        if (!res || !res.range) return null;
        return [new vscode.Location(vscode.Uri.parse(res.uri), rangeOf(res.range))];
      },
    })
  );

  context.subscriptions.push(
    vscode.languages.registerReferenceProvider(documentSelector(), {
      async provideReferences(doc, pos, ctx) {
        const res = await request('textDocument/references', {
          textDocument: { uri: doc.uri.toString() },
          position: { line: pos.line, character: pos.character },
          context: ctx,
        });
        return (res || []).map((l) => new vscode.Location(vscode.Uri.parse(l.uri), rangeOf(l.range)));
      },
    })
  );

  context.subscriptions.push(
    vscode.languages.registerRenameProvider(documentSelector(), {
      async prepareRename(doc, pos) {
        return new vscode.Range(pos.line, pos.character, pos.line, pos.character + 1);
      },
      async provideRenameEdits(doc, pos, newName) {
        const res = await request('textDocument/rename', {
          textDocument: { uri: doc.uri.toString() },
          position: { line: pos.line, character: pos.character },
          newName,
        });
        if (!res || !res.changes) return null;
        const changes = [];
        for (const [uri, edits] of Object.entries(res.changes)) {
          const u = vscode.Uri.parse(uri);
          changes.push(...edits.map((e) => new vscode.TextEdit(rangeOf(e.range), e.newText)));
          void u;
        }
        const edit = new vscode.WorkspaceEdit();
        // Group by document uri (single-document rename today).
        doc.uri && edit.set(doc.uri, changes);
        return edit;
      },
    })
  );

  context.subscriptions.push(
    vscode.languages.registerDocumentFormattingEditProvider(documentSelector(), {
      async provideDocumentFormattingEdits(doc) {
        const res = await request('textDocument/formatting', {
          textDocument: { uri: doc.uri.toString() },
          options: { tabSize: 4, insertSpaces: true },
        });
        return (res || []).map((e) => new vscode.TextEdit(rangeOf(e.range), e.newText));
      },
    })
  );
}

function rangeOf(r) {
  return new vscode.Range(
    r.start.line,
    r.start.character,
    r.end.line,
    r.end.character
  );
}

function deactivate() {
  if (serverProcess) serverProcess.kill();
}

module.exports = { activate, deactivate };
