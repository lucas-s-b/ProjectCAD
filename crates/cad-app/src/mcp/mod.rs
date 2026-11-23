//! Model Context Protocol server over stdin/stdout (JSON-RPC 2.0, one message per line).
pub mod bridge;
pub mod tools;
pub mod widget_req;

use eframe::egui;
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::sync::mpsc;
use std::time::Duration;

#[derive(Debug)]
pub enum Request {
    Screenshot,
    State,
    Click { x: f32, y: f32, button: egui::PointerButton, world: bool, mods: egui::Modifiers },
    Drag { x1: f32, y1: f32, x2: f32, y2: f32, button: egui::PointerButton, world: bool, mods: egui::Modifiers },
    Scroll { x: f32, y: f32, dy: f32, world: bool },
    Key { key: egui::Key, mods: egui::Modifiers },
    Text(String),
    Widgets,
    ClickWidget { name: String, index: usize, mods: egui::Modifiers },
    SetText { name: String, index: usize, text: String },
    New,
    Open(std::path::PathBuf),
    Import(std::path::PathBuf),
    Save(Option<std::path::PathBuf>),
    Export(std::path::PathBuf),
    ExportMotion(std::path::PathBuf),
    Apply(Vec<cad_document::Command>),
}

pub struct Reply {
    pub text: String,
    pub png: Option<Vec<u8>>,
}

pub type Job = (Request, mpsc::Sender<Result<Reply, String>>);

const TIMEOUT: Duration = Duration::from_secs(30);

/// MCP over the app's own stdin/stdout (`--mcp`), every tool. The app exits when stdin closes.
pub fn spawn(ctx: egui::Context) -> bridge::Bridge {
    let (tx, rx) = mpsc::channel::<Job>();
    std::thread::spawn(move || {
        serve(std::io::stdin().lock(), &mut std::io::stdout(), &ctx, &tx, |_| true);
        std::process::exit(0);
    });
    bridge::Bridge::new(rx, true)
}

/// MCP for a plugin started from the Plugins menu: requests come from `from` (the plugin's stdout), replies go to
/// `to` (its stdin); only `tools::plugin_allowed` tools. The thread ends when the plugin closes its stdout; the app
/// keeps running.
pub fn spawn_plugin(ctx: egui::Context, from: impl std::io::Read + Send + 'static, mut to: impl Write + Send + 'static) -> bridge::Bridge {
    let (tx, rx) = mpsc::channel::<Job>();
    std::thread::spawn(move || {
        serve(std::io::BufReader::new(from), &mut to, &ctx, &tx, tools::plugin_allowed);
        ctx.request_repaint();
    });
    bridge::Bridge::new(rx, false)
}

/// Answers MCP messages read from `input` on `out` until `input` ends; only tools that `allowed` accepts are listed
/// and run.
fn serve(input: impl BufRead, out: &mut dyn Write, ctx: &egui::Context, jobs: &mpsc::Sender<Job>, allowed: fn(&str) -> bool) {
    for line in input.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                write_line(out, &json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": e.to_string()}}));
                continue;
            }
        };
        let reply = if msg["method"] == "tools/call" {
            msg.get("id").map(|id| json!({"jsonrpc": "2.0", "id": id, "result": call(ctx, jobs, &msg["params"], allowed)}))
        } else {
            handle_local(&msg, allowed)
        };
        if let Some(reply) = reply {
            write_line(out, &reply);
        }
    }
}

fn call(ctx: &egui::Context, jobs: &mpsc::Sender<Job>, params: &Value, allowed: fn(&str) -> bool) -> Value {
    let name = params["name"].as_str().unwrap_or("");
    let outcome = if allowed(name) {
        tools::parse(name, &params["arguments"]).and_then(|req| {
            let (tx, rx) = mpsc::channel();
            jobs.send((req, tx)).map_err(|_| "application is closing".to_string())?;
            ctx.request_repaint();
            rx.recv_timeout(TIMEOUT)
                .map_err(|_| "application did not respond; is the window minimized?".to_string())?
        })
    } else {
        Err(format!("tool '{name}' is not available to plugins started from the Plugins menu"))
    };
    match outcome {
        Ok(r) => {
            let mut content = vec![json!({"type": "text", "text": r.text})];
            if let Some(png) = r.png {
                content.push(json!({"type": "image", "data": base64(&png), "mimeType": "image/png"}));
            }
            json!({"content": content, "isError": false})
        }
        Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
    }
}

fn write_line(out: &mut dyn Write, v: &Value) {
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}

/// Answers everything except `tools/call`. None for notifications. `tools/list` shows only tools `allowed` accepts.
fn handle_local(msg: &Value, allowed: fn(&str) -> bool) -> Option<Value> {
    let id = msg.get("id")?;
    let reply = |result: Value| json!({"jsonrpc": "2.0", "id": id, "result": result});
    Some(match msg["method"].as_str().unwrap_or("") {
        "initialize" => reply(json!({
            "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "projectcad", "version": env!("CARGO_PKG_VERSION")},
        })),
        "ping" => reply(json!({})),
        "tools/list" => {
            let all = tools::definitions();
            let list: Vec<&Value> = all.as_array().into_iter().flatten().filter(|t| t["name"].as_str().is_some_and(allowed)).collect();
            reply(json!({"tools": list}))
        }
        _ => json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "method not found"}}),
    })
}

pub fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                s.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_vectors() {
        for (i, o) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foobar", "Zm9vYmFy")] {
            assert_eq!(base64(i.as_bytes()), o);
        }
    }

    #[test]
    fn local_methods() {
        let all = |_: &str| true;
        let r = handle_local(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}}), all).unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
        let r = handle_local(&json!({"id": 2, "method": "initialize"}), all).unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(handle_local(&json!({"id": 3, "method": "ping"}), all).unwrap()["result"], json!({}));
        let r = handle_local(&json!({"id": 4, "method": "tools/list"}), all).unwrap();
        let list = r["result"]["tools"].as_array().unwrap();
        assert_eq!(list.len(), 17);
        for t in list {
            assert!(t["name"].is_string() && t["description"].is_string() && t["inputSchema"].is_object());
        }
        assert!(handle_local(&json!({"method": "notifications/initialized"}), all).is_none());
        let r = handle_local(&json!({"id": 5, "method": "nope"}), all).unwrap();
        assert_eq!(r["error"]["code"], -32601);
    }

    #[test]
    fn plugin_tool_list() {
        let r = handle_local(&json!({"id": 1, "method": "tools/list"}), tools::plugin_allowed).unwrap();
        let names: Vec<&str> = r["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
        assert_eq!(names, ["screenshot", "state", "export_project", "export_motion", "widgets", "apply_commands"]);
    }
}
