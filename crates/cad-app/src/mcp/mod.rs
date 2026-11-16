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
}

pub struct Reply {
    pub text: String,
    pub png: Option<Vec<u8>>,
}

pub type Job = (Request, mpsc::Sender<Result<Reply, String>>);

const TIMEOUT: Duration = Duration::from_secs(30);

pub fn spawn(ctx: egui::Context) -> bridge::Bridge {
    let (tx, rx) = mpsc::channel::<Job>();
    std::thread::spawn(move || reader(&ctx, &tx));
    bridge::Bridge::new(rx)
}

fn reader(ctx: &egui::Context, jobs: &mpsc::Sender<Job>) {
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                write_line(&json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": e.to_string()}}));
                continue;
            }
        };
        let out = if msg["method"] == "tools/call" {
            msg.get("id").map(|id| json!({"jsonrpc": "2.0", "id": id, "result": call(ctx, jobs, &msg["params"])}))
        } else {
            handle_local(&msg)
        };
        if let Some(out) = out {
            write_line(&out);
        }
    }
    std::process::exit(0);
}

fn call(ctx: &egui::Context, jobs: &mpsc::Sender<Job>, params: &Value) -> Value {
    let name = params["name"].as_str().unwrap_or("");
    let outcome = tools::parse(name, &params["arguments"]).and_then(|req| {
        let (tx, rx) = mpsc::channel();
        jobs.send((req, tx)).map_err(|_| "application is closing".to_string())?;
        ctx.request_repaint();
        rx.recv_timeout(TIMEOUT)
            .map_err(|_| "application did not respond; is the window minimized?".to_string())?
    });
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

fn write_line(v: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}

/// Answers everything except `tools/call`. None for notifications.
fn handle_local(msg: &Value) -> Option<Value> {
    let id = msg.get("id")?;
    let reply = |result: Value| json!({"jsonrpc": "2.0", "id": id, "result": result});
    Some(match msg["method"].as_str().unwrap_or("") {
        "initialize" => reply(json!({
            "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "projectcad", "version": env!("CARGO_PKG_VERSION")},
        })),
        "ping" => reply(json!({})),
        "tools/list" => reply(json!({"tools": tools::definitions()})),
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
        let r = handle_local(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}})).unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
        let r = handle_local(&json!({"id": 2, "method": "initialize"})).unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(handle_local(&json!({"id": 3, "method": "ping"})).unwrap()["result"], json!({}));
        let r = handle_local(&json!({"id": 4, "method": "tools/list"})).unwrap();
        let list = r["result"]["tools"].as_array().unwrap();
        assert_eq!(list.len(), 15);
        for t in list {
            assert!(t["name"].is_string() && t["description"].is_string() && t["inputSchema"].is_object());
        }
        assert!(handle_local(&json!({"method": "notifications/initialized"})).is_none());
        let r = handle_local(&json!({"id": 5, "method": "nope"})).unwrap();
        assert_eq!(r["error"]["code"], -32601);
    }
}
