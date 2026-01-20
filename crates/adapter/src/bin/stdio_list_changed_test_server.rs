//! Minimal MCP stdio server used only for adapter integration tests.
//!
//! This server can mutate its exposed tool list at runtime and emits
//! `notifications/tools/list_changed` to test the Adapter's dynamic refresh behavior.

use serde_json::json;
use std::io::{BufRead as _, Write};

fn main() -> anyhow::Result<()> {
    let mut state = ServerState::default();
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        for out in handle_line(&mut state, &line) {
            write_json_line(&mut stdout, &out)?;
        }
    }

    Ok(())
}

#[derive(Default)]
struct ServerState {
    extra_tool_enabled: bool,
}

fn handle_line(state: &mut ServerState, line: &str) -> Vec<serde_json::Value> {
    let line = line.trim();
    if line.is_empty() {
        return Vec::new();
    }

    let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
        return Vec::new();
    };

    handle_message(state, &msg)
}

fn handle_message(state: &mut ServerState, msg: &serde_json::Value) -> Vec<serde_json::Value> {
    let method = msg.get("method").and_then(serde_json::Value::as_str);
    let Some(method) = method else {
        return Vec::new();
    };

    // Ignore notifications (no `id`).
    let Some(id) = msg.get("id").cloned() else {
        return Vec::new();
    };

    match method {
        "initialize" => vec![jsonrpc_ok(&id, &initialize_result(msg))],
        "resources/list" => vec![jsonrpc_ok(&id, &json!({ "resources": [] }))],
        "prompts/list" => vec![jsonrpc_ok(&id, &json!({ "prompts": [] }))],
        "tools/list" => vec![jsonrpc_ok(&id, &tools_list_result(state))],
        "tools/call" => tools_call(state, msg, &id),
        _ => vec![jsonrpc_err(
            &id,
            &json!({ "code": -32601, "message": "method not found" }),
        )],
    }
}

fn initialize_result(msg: &serde_json::Value) -> serde_json::Value {
    let protocol_version = msg
        .get("params")
        .and_then(|p| p.get("protocolVersion"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("2024-11-05")
        .to_string();

    json!({
        "protocolVersion": protocol_version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "adapter-stdio-list-changed-test-server", "version": "0" }
    })
}

fn tools_list_result(state: &ServerState) -> serde_json::Value {
    let mut tools = vec![
        json!({
            "name": "toggle_extra_tool",
            "description": "Enable an extra tool and emit notifications/tools/list_changed",
            "inputSchema": { "type": "object" }
        }),
        json!({
            "name": "whoami",
            "description": "A stable tool always present",
            "inputSchema": { "type": "object" }
        }),
    ];

    if state.extra_tool_enabled {
        tools.push(json!({
            "name": "hello",
            "description": "Appears after toggle_extra_tool is called",
            "inputSchema": { "type": "object" }
        }));
    }

    json!({ "tools": tools })
}

fn tools_call(
    state: &mut ServerState,
    msg: &serde_json::Value,
    id: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let name = msg
        .get("params")
        .and_then(|p| p.get("name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    match name {
        "toggle_extra_tool" => {
            if !state.extra_tool_enabled {
                state.extra_tool_enabled = true;
            }

            // Emit tool list changed notification before returning the response.
            let notify = json!({
                "jsonrpc": "2.0",
                "method": "notifications/tools/list_changed"
            });

            let result = json!({
                "content": [{ "type": "text", "text": "ok" }]
            });
            vec![notify, jsonrpc_ok(id, &result)]
        }
        "whoami" => {
            let result = json!({
                "content": [{ "type": "text", "text": "whoami" }]
            });
            vec![jsonrpc_ok(id, &result)]
        }
        "hello" => {
            if !state.extra_tool_enabled {
                return vec![jsonrpc_err(
                    id,
                    &json!({ "code": -32601, "message": "unknown tool" }),
                )];
            }
            let result = json!({
                "content": [{ "type": "text", "text": "hello" }]
            });
            vec![jsonrpc_ok(id, &result)]
        }
        _ => vec![jsonrpc_err(
            id,
            &json!({ "code": -32601, "message": "unknown tool" }),
        )],
    }
}

fn jsonrpc_ok(id: &serde_json::Value, result: &serde_json::Value) -> serde_json::Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn jsonrpc_err(id: &serde_json::Value, error: &serde_json::Value) -> serde_json::Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": error })
}

fn write_json_line(stdout: &mut dyn Write, v: &serde_json::Value) -> anyhow::Result<()> {
    writeln!(stdout, "{}", serde_json::to_string(v)?)?;
    stdout.flush()?;
    Ok(())
}
