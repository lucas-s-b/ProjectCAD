use cad_document::{Command, FORMAT_VERSION, NextIds};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Stdio};

/// What can go wrong talking to the app.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("the app closed the connection")]
    Closed,
    #[error("protocol: {0}")]
    Protocol(String),
    #[error("JSON-RPC error {code}: {message}")]
    Rpc { code: i64, message: String },
    /// The tool ran and refused (the app's message, e.g. "entity 3 already exists").
    #[error("{0}")]
    Tool(String),
    #[error("the app uses command format {app}, this plugin was built for {plugin}")]
    Version { app: u32, plugin: u32 },
}

/// The content of a tool reply.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolReply {
    pub text: String,
    /// Decoded image data (PNG), from `screenshot`.
    pub image: Option<Vec<u8>>,
}

/// The short reply of the input and file tools (the app sends more fields; these are the stable ones).
#[derive(Clone, Debug, Deserialize)]
pub struct Summary {
    pub tool: String,
    pub view: String,
    pub status: String,
    pub entities: usize,
    pub dirty: bool,
    pub selection: Vec<u64>,
}

/// One named widget of the last frame, in screenshot pixels.
#[derive(Clone, Debug, Deserialize)]
pub struct Widget {
    pub name: String,
    pub index: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub visible: bool,
}

/// The reply of `apply_commands`.
#[derive(Clone, Debug, Deserialize)]
pub struct Applied {
    pub applied: usize,
    /// Feature count: `Command::AddFeature { index }` with this index appends.
    pub features: usize,
    pub format_version: u32,
    pub next: NextIds,
    /// The inverse commands of the step (empty for an empty list).
    pub undo: Vec<Command>,
}

pub struct Screenshot {
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Button {
    #[default]
    Primary,
    Secondary,
    Middle,
}

/// Options of `click` and `drag`; `world`: x and y are millimetres in the 2D view.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pointer {
    pub button: Button,
    pub world: bool,
    pub shift: bool,
    pub ctrl: bool,
}

/// A connection to the app.
pub struct Client {
    reader: Box<dyn BufRead>,
    writer: Box<dyn Write>,
    child: Option<Child>,
    next_id: u64,
}

impl Client {
    /// True when this process was started from the app's Plugins menu (`CAD_PLUGIN` set).
    pub fn launched_by_app() -> bool {
        std::env::var_os("CAD_PLUGIN").is_some()
    }

    /// Starts `<app> --mcp --background` (an off-screen window that takes no focus and no OS input) and connects to it.
    pub fn spawn(app: &Path) -> Result<Client, PluginError> {
        let mut child = std::process::Command::new(app)
            .args(["--mcp", "--background"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let (Some(from), Some(to)) = (child.stdout.take(), child.stdin.take()) else {
            let _ = child.kill();
            return Err(PluginError::Protocol("no pipes to the app".into()));
        };
        Client::connect(Box::new(BufReader::new(from)), Box::new(to), Some(child))
    }

    /// Connects over this process's stdin/stdout: how a plugin started from the Plugins menu talks to the app.
    pub fn stdio() -> Result<Client, PluginError> {
        Client::connect(Box::new(BufReader::new(std::io::stdin())), Box::new(std::io::stdout()), None)
    }

    /// Connects over any pair of streams: `initialize`, `notifications/initialized`, then an empty `apply_commands`
    /// whose `format_version` must equal `FORMAT_VERSION`.
    pub fn connect(reader: Box<dyn BufRead>, writer: Box<dyn Write>, child: Option<Child>) -> Result<Client, PluginError> {
        let mut client = Client { reader, writer, child, next_id: 1 };
        let id = client.next_id();
        let init = json!({"jsonrpc": "2.0", "id": id, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "cad-plugin-api", "version": env!("CARGO_PKG_VERSION")}}});
        client.send(&init.to_string())?;
        client.wait_for(id)?;
        client.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string())?;
        let q = client.query()?;
        if q.format_version != FORMAT_VERSION {
            return Err(PluginError::Version { app: q.format_version, plugin: FORMAT_VERSION });
        }
        Ok(client)
    }

    fn next_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id - 1
    }

    fn send(&mut self, line: &str) -> Result<(), PluginError> {
        writeln!(self.writer, "{line}")?;
        self.writer.flush()?;
        Ok(())
    }

    /// Reads lines until the reply to `id` arrives.
    fn wait_for(&mut self, id: u64) -> Result<Value, PluginError> {
        loop {
            let mut line = String::new();
            if self.reader.read_line(&mut line)? == 0 {
                return Err(PluginError::Closed);
            }
            if line.trim().is_empty() {
                continue;
            }
            if let Some(result) = parse_message(&line, id)? {
                return Ok(result);
            }
        }
    }

    /// Calls `tool` with `args` and waits for its reply (messages with other ids are skipped).
    pub fn call(&mut self, tool: &str, args: Value) -> Result<ToolReply, PluginError> {
        let id = self.next_id();
        self.send(&request_line(id, tool, &args))?;
        let result = self.wait_for(id)?;
        tool_reply(&result)
    }

    fn typed<T: serde::de::DeserializeOwned>(&mut self, tool: &str, args: Value) -> Result<T, PluginError> {
        let reply = self.call(tool, args)?;
        serde_json::from_str(&reply.text).map_err(|err| PluginError::Protocol(format!("{tool} reply: {err}")))
    }

    /// The whole state (see the app's `state` tool description); too large to type, so JSON.
    pub fn state(&mut self) -> Result<Value, PluginError> {
        self.typed("state", json!({}))
    }

    pub fn screenshot(&mut self) -> Result<Screenshot, PluginError> {
        let reply = self.call("screenshot", json!({}))?;
        let bad = || PluginError::Protocol(format!("screenshot size '{}'", reply.text));
        let (w, h) = reply.text.split_once('x').ok_or_else(bad)?;
        let (width, height) = (w.trim().parse().map_err(|_| bad())?, h.trim().parse().map_err(|_| bad())?);
        let png = reply.image.clone().ok_or_else(|| PluginError::Protocol("screenshot without an image".into()))?;
        Ok(Screenshot { width, height, png })
    }

    pub fn widgets(&mut self) -> Result<Vec<Widget>, PluginError> {
        self.typed("widgets", json!({}))
    }

    pub fn click(&mut self, x: f32, y: f32, p: Pointer) -> Result<Summary, PluginError> {
        let mut args = pointer_args(&p);
        args.extend([("x".to_string(), json!(x)), ("y".to_string(), json!(y))]);
        self.typed("click", Value::Object(args))
    }

    pub fn drag(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, p: Pointer) -> Result<Summary, PluginError> {
        let mut args = pointer_args(&p);
        args.extend([("x1".to_string(), json!(x1)), ("y1".to_string(), json!(y1)), ("x2".to_string(), json!(x2)), ("y2".to_string(), json!(y2))]);
        self.typed("drag", Value::Object(args))
    }

    pub fn scroll(&mut self, x: f32, y: f32, dy: f32, world: bool) -> Result<Summary, PluginError> {
        self.typed("scroll", json!({"x": x, "y": y, "dy": dy, "world": world}))
    }

    /// `key` is an egui key name such as "Enter", "Escape", "Z".
    pub fn key(&mut self, key: &str, shift: bool, ctrl: bool) -> Result<Summary, PluginError> {
        self.typed("key", json!({"key": key, "shift": shift, "ctrl": ctrl}))
    }

    pub fn type_text(&mut self, text: &str) -> Result<Summary, PluginError> {
        self.typed("type_text", json!({"text": text}))
    }

    pub fn click_widget(&mut self, name: &str, index: usize, shift: bool, ctrl: bool) -> Result<Summary, PluginError> {
        self.typed("click_widget", json!({"name": name, "index": index, "shift": shift, "ctrl": ctrl}))
    }

    pub fn set_text(&mut self, name: &str, index: usize, text: &str) -> Result<Summary, PluginError> {
        self.typed("set_text", json!({"name": name, "index": index, "text": text}))
    }

    pub fn new_project(&mut self) -> Result<Summary, PluginError> {
        self.typed("new_project", json!({}))
    }

    pub fn open_project(&mut self, path: &Path) -> Result<Summary, PluginError> {
        self.typed("open_project", json!({"path": path_str(path)?}))
    }

    pub fn import_project(&mut self, path: &Path) -> Result<Summary, PluginError> {
        self.typed("import_project", json!({"path": path_str(path)?}))
    }

    /// `None` saves to the current file.
    pub fn save_project(&mut self, path: Option<&Path>) -> Result<Summary, PluginError> {
        let args = match path {
            Some(p) => json!({"path": path_str(p)?}),
            None => json!({}),
        };
        self.typed("save_project", args)
    }

    pub fn export_project(&mut self, path: &Path) -> Result<Summary, PluginError> {
        self.typed("export_project", json!({"path": path_str(path)?}))
    }

    pub fn export_motion(&mut self, path: &Path) -> Result<Summary, PluginError> {
        self.typed("export_motion", json!({"path": path_str(path)?}))
    }

    /// Applies `cmds` as one undo step (all or nothing).
    pub fn apply_commands(&mut self, cmds: &[Command]) -> Result<Applied, PluginError> {
        let commands = serde_json::to_value(cmds).map_err(|err| PluginError::Protocol(err.to_string()))?;
        self.typed("apply_commands", json!({"commands": commands}))
    }

    /// The next free ids and the feature count, changing nothing (`apply_commands` of an empty list).
    pub fn query(&mut self) -> Result<Applied, PluginError> {
        self.apply_commands(&[])
    }
}

impl Drop for Client {
    /// A spawned app: closes its stdin (the app exits at end of input) and waits for it.
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            self.writer = Box::new(std::io::sink());
            let _ = child.wait();
        }
    }
}

fn path_str(p: &Path) -> Result<&str, PluginError> {
    p.to_str().ok_or_else(|| PluginError::Protocol("path is not UTF-8".into()))
}

/// The JSON-RPC line (no newline) of tool call `id`.
fn request_line(id: u64, tool: &str, args: &Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": tool, "arguments": args}}).to_string()
}

/// The `result` of the reply to `id` in `line`; Ok(None) for any other message (notifications, other ids);
/// Err(Rpc) for a JSON-RPC error, Err(Protocol) for a line that is not JSON.
fn parse_message(line: &str, id: u64) -> Result<Option<Value>, PluginError> {
    let msg: Value = serde_json::from_str(line).map_err(|err| PluginError::Protocol(format!("not JSON ({err}): {}", line.trim())))?;
    if msg.get("id").and_then(Value::as_u64) != Some(id) {
        return Ok(None);
    }
    if let Some(err) = msg.get("error") {
        let code = err.get("code").and_then(Value::as_i64).unwrap_or(0);
        let message = err.get("message").and_then(Value::as_str).unwrap_or("").to_string();
        return Err(PluginError::Rpc { code, message });
    }
    msg.get("result").cloned().map(Some).ok_or_else(|| PluginError::Protocol("reply without result".into()))
}

/// The text and decoded image of a tool `result`; Err(Tool(text)) when `isError` is true.
fn tool_reply(result: &Value) -> Result<ToolReply, PluginError> {
    let mut text = String::new();
    let mut image = None;
    for part in result.get("content").and_then(Value::as_array).into_iter().flatten() {
        match part.get("type").and_then(Value::as_str) {
            Some("text") => text.push_str(part.get("text").and_then(Value::as_str).unwrap_or("")),
            Some("image") => image = Some(base64_decode(part.get("data").and_then(Value::as_str).unwrap_or(""))?),
            _ => {}
        }
    }
    if result.get("isError").and_then(Value::as_bool).unwrap_or(false) {
        return Err(PluginError::Tool(text));
    }
    Ok(ToolReply { text, image })
}

/// Standard base64 with padding (the app's encoder); Err(Protocol) on any other character.
fn base64_decode(s: &str) -> Result<Vec<u8>, PluginError> {
    let bad = || PluginError::Protocol("bad base64".into());
    let bytes = s.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(bad());
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let pad = chunk.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 {
            return Err(bad());
        }
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            let v = match c {
                b'A'..=b'Z' => c - b'A',
                b'a'..=b'z' => c - b'a' + 26,
                b'0'..=b'9' => c - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' if i >= 4 - pad => 0,
                _ => return Err(bad()),
            };
            n = (n << 6) | u32::from(v);
        }
        out.extend_from_slice(&n.to_be_bytes()[1..4 - pad]);
    }
    Ok(out)
}

/// The `button`, `world`, `shift` and `ctrl` arguments of `p` ("primary", "secondary", "middle").
fn pointer_args(p: &Pointer) -> serde_json::Map<String, Value> {
    let button = match p.button {
        Button::Primary => "primary",
        Button::Secondary => "secondary",
        Button::Middle => "middle",
    };
    let Value::Object(m) = json!({"button": button, "world": p.world, "shift": p.shift, "ctrl": p.ctrl}) else { unreachable!() };
    m
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
