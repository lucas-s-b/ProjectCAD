# Plugin developer guide

## 1. What a plugin is

A plugin is a separate program that talks to ProjectCAD over MCP (the Model Context Protocol): JSON-RPC 2.0, one JSON
message per line. The app is the server, the plugin is the client. Any language works, because the connection is plain
JSON over a pipe. A typed Rust client, the crate `cad-plugin-api`, is provided; this guide uses it for its examples.

There is no ABI and no `unsafe`: a plugin is an ordinary program. The app serves the same tools it serves to any MCP
client (`state`, `screenshot`, `widgets`, `click`, ..., and `apply_commands`, which edits the document).

## 2. Two ways to run

**From the Plugins menu.** The user clicks "Plugin: <name>" in the Plugins menu. The app starts the program as a child
process, in the plugins folder, with the environment variable `CAD_PLUGIN=1`. The program's stdin and stdout are the
connection (the app writes to the plugin's stdin and reads its stdout). Only the document tools are offered: `state`,
`screenshot`, `widgets`, `apply_commands`, `export_project` and `export_motion`. Input tools (`click`, `drag`, `key`, ...)
would fight the user's own mouse and keyboard in the same window, and new, open, import and save would skip the
unsaved-changes dialog. A call to another tool is answered with an error. One plugin runs at a time; "Stop plugin"
kills it. **Never print to stdout in a menu-started run**: stdout is the protocol. Write messages to stderr.

**Standalone.** The program starts `cad-app --mcp --background` itself (an off-screen window that takes no focus and no
OS input) and talks to it over that process's pipes. Every tool is available, and the program saves its result with
`save_project`. No running app is needed. In Rust: `Client::spawn(path_to_cad_app)`.

## 3. Installing

Plugins live in the folder `plugins` beside the app's settings file:

- Windows: `%APPDATA%\ProjectCAD\plugins`
- elsewhere: `$XDG_CONFIG_HOME/projectcad/plugins` or `~/.config/projectcad/plugins`
- or beside the file named by the environment variable `CAD_SETTINGS`.

Each plugin is one `plugins/<file>.json` manifest:

```json
{"name": "Bolt circle", "command": "C:/tools/bolt_circle.exe", "args": ["--count", "8"], "version": "1"}
```

- `name`: the menu entry "Plugin: <name>"; unique, 1 to 64 characters.
- `command`: the program, absolute or relative to the plugins folder. It is started directly, never through a shell.
- `args`: optional list of arguments, fixed in the manifest (there is no parameter dialog).
- `version`: free text.

Unknown fields are refused. Manifests are read at startup and by "Reload plugins" in the Plugins menu; a file that
cannot be used is listed in the menu with the reason ("plugins/<file>: <reason>").

## 4. `apply_commands`

`apply_commands` takes `{"commands": [...]}`. Each command is the serde JSON of `cad_document::Command`, externally
tagged: the variant name is the key.

```json
{"Remove": 3}
{"SetParam": {"name": "w", "expr": "5 mm"}}
{"Add": {"id": 5, "name": "c", "shape": {"Circle": {"center": [0, 0], "radius": 2}}}}
{"AddFeature": {"index": 0, "feature": {"id": 1, "name": "Disc", "suppressed": false, "kind": {"Extrude": {
    "profile": [5], "distance": "10 mm", "offset": "0", "taper": "0 deg", "op": "New", "flip": false, "target": "Auto"}}}}}
{"AddConstraint": [4, {"...": "tuple variants are arrays: the id, then the constraint"}]}
```

The payloads are the project file's types, so fields that are omitted and have a default in the project file take that
default (an `Add` needs only `id`, `name` and `shape`).

- **Ids.** Commands carry their own ids, as the UI's do. Call `apply_commands` with an empty list first: it changes
  nothing and replies the next free id of every kind (`next`) and the feature count (`features`). Number your objects
  from `next`; append features with `AddFeature` at index `features`. If the user added something in between, the
  collision is refused whole ("entity 3 already exists") and you may ask again.
- **One undo step, all or nothing.** The list runs in order as one undo step (Ctrl+Z reverts it). Any error rolls every
  command back and leaves the document exactly as it was. The constraint solve and the dimension refresh happen as after
  any edit in the UI.
- **Errors.** A command that does not parse is reported as "command <i>: <serde message>" (for example
  "command 1: unknown variant `Explode`" or "command 0: missing field `name`"); the argument object holds only
  `commands` ("unknown argument '<k>'"); at most 10 000 commands per call; then the document validates each command
  ("entity 3 already exists", "entity 99 not found", ...). The reply of a refused call is a tool error.
- **Reply.** `{"applied": n, "features": count, "format_version": 28, "next": {"component", "constraint", "entity",
  "feature", "group", "joint", "layer", "sketch"}, "undo": [...]}`; `undo` holds the inverse commands of the step (empty
  for an empty list).
- **Versioning.** The schema is the project file's types, so it changes exactly when `FORMAT_VERSION` changes (28 now).
  The Rust client checks `format_version` when it connects and refuses an app with another version: rebuild the plugin
  after a format change.

## 5. The Rust client

Add the crate as a path dependency (it re-exports `cad_document` and `cad_geometry`, so it is the only one you need):

```toml
[dependencies]
cad-plugin-api = { path = "../ProjectCAD/crates/cad-plugin-api" }
```

```rust
use cad_plugin_api::{Client, Command};

fn main() -> Result<(), cad_plugin_api::PluginError> {
    let mut client = if Client::launched_by_app() {
        Client::stdio()?
    } else {
        Client::spawn(std::path::Path::new("target/debug/cad-app.exe"))?
    };
    let q = client.query()?;                       // next free ids and the feature count
    let cmds: Vec<Command> = vec![/* numbered from q.next, features at index q.features */];
    let done = client.apply_commands(&cmds)?;      // one undo step
    eprintln!("next entity id is now {}", done.next.entity);
    Ok(())
}
```

`Client` has one method per tool (`state`, `screenshot`, `widgets`, `click`, `drag`, `scroll`, `key`, `type_text`,
`click_widget`, `set_text`, `new_project`, `open_project`, `import_project`, `save_project`, `export_project`,
`export_motion`, `apply_commands`, `query`) and a generic `call(tool, json)`. Errors are `PluginError`:

- `Io`: reading or writing the pipe failed.
- `Closed`: the app closed the connection.
- `Protocol`: a line that is not JSON, or a reply of an unexpected shape.
- `Rpc { code, message }`: a JSON-RPC error (for example an unknown method).
- `Tool(message)`: the tool ran and refused (the app's message).
- `Version { app, plugin }`: the app uses another command format.

A spawned app is closed when the `Client` is dropped (its stdin closes and the app exits at end of input).

## 6. The example

The crate ships the example plugin `bolt_circle`: a round flange (outer radius 60 mm, 10 mm thick) with a circle of bolt
holes (default 6 holes, 8 mm diameter on a 40 mm radius), as one disc extrude and one cutting extrude per hole. Its
model lives in the library (`cad_plugin_api::bolt_circle`), so its unit tests run with `cargo test`.

```
cargo build -p cad-plugin-api --examples
```

Menu manifest for `target/debug/examples/bolt_circle.exe` with 8 holes:

```json
{"name": "Bolt circle", "command": "D:/path/to/ProjectCAD/target/debug/examples/bolt_circle.exe", "args": ["--count", "8"], "version": "1"}
```

Standalone (starts the app, saves the project, needs no running app):

```
bolt_circle --app target/debug/cad-app.exe --save flange.cadproj [--count 8] [--bolt-radius 40] [--hole-diameter 8] [--outer-radius 60] [--thickness 10]
```

The volume is pi * 10 * (60^2 - n * 4^2): 110 081.4 mm^3 for 6 holes, 109 076.1 mm^3 for 8 holes (the tessellated
solid is within about 1 % of it). The example refuses sizes whose holes touch each other, the rim or the centre.

## 7. Security

A plugin is a program of the user's and runs with the user's full rights. The app adds no sandbox: install only programs
you trust. The app never starts a plugin by itself, never reads plugins from a project or its folder, and only a click in
the Plugins menu starts one. MCP runs without `CAD_SETTINGS` have no plugins folder at all, so they stay deterministic.
Each `apply_commands` call is one undo step (Ctrl+Z reverts it). "Stop plugin" kills the process. Plugin arguments are
passed to the program directly, never through a shell, so a manifest cannot inject a command line.

## 8. Limits

- Plugins run with the user's rights, without a sandbox; only a click in the Plugins menu starts one, never a project file.
- One plugin at a time; its arguments are fixed in its manifest (no parameter dialog); menu-started plugins cannot click,
  type, open or save.
- A plugin built for another project file format version is refused (rebuild it after a format change).
- There is no way to connect from outside to an already running GUI app, no plugin UI inside the app, no folder watcher
  (use "Reload plugins"), and only the Rust client ships (other languages speak the JSON directly).
