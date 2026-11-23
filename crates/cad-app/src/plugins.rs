//! Plugins: programs listed in `plugins/<file>.json` beside the settings file, started from the Plugins menu. A plugin
//! runs as a child process with the user's rights and speaks MCP over its stdin/stdout with the app as the server
//! (document tools only, `mcp::tools::plugin_allowed`). The app never starts one by itself, and never from a project.
// shortcut: a plugin's arguments are fixed in its manifest (no parameter dialog); add one if plugins need input per run
use crate::app::CadApp;
use crate::mcp::bridge::Bridge;
use eframe::egui;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// Set to "1" in a plugin's environment; `cad-plugin-api` reads it (`Client::launched_by_app`).
pub const PLUGIN_ENV: &str = "CAD_PLUGIN";

/// One `plugins/<file>.json`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// The menu entry "Plugin: <name>"; unique, 1 to 64 characters.
    pub name: String,
    /// The program: absolute, or relative to the plugins folder. Started directly, never through a shell.
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub version: String,
}

/// A plugin while it runs.
pub struct Running {
    pub name: String,
    child: Child,
    pub bridge: Bridge,
}

/// The plugins folder: `plugins` beside the settings file; None without one (MCP runs without `CAD_SETTINGS`).
pub fn dir(settings: Option<&Path>) -> Option<PathBuf> {
    settings.and_then(Path::parent).map(|p| p.join("plugins"))
}

/// The manifests in `dir` (`*.json`, read in file-name order; a missing folder has none), sorted by name, and one
/// "plugins/<file>: <reason>" per file that cannot be read or parsed, has an empty name or one over 64 characters,
/// or repeats an earlier name ("duplicate plugin name '<name>'").
pub fn scan(dir: &Path) -> (Vec<Manifest>, Vec<String>) {
    let Ok(read) = std::fs::read_dir(dir) else { return (Vec::new(), Vec::new()) };
    let mut files: Vec<PathBuf> =
        read.filter_map(Result::ok).map(|el| el.path()).filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "json")).collect();
    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    let (mut found, mut errors): (Vec<Manifest>, Vec<String>) = (Vec::new(), Vec::new());
    for path in files {
        let file = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let parsed = std::fs::read_to_string(&path).map_err(|err| err.to_string()).and_then(|text| serde_json::from_str::<Manifest>(&text).map_err(|err| err.to_string()));
        match parsed {
            Err(reason) => errors.push(format!("plugins/{file}: {reason}")),
            Ok(m) if m.name.is_empty() || m.name.chars().count() > 64 => {
                errors.push(format!("plugins/{file}: the name must have 1 to 64 characters"));
            }
            Ok(m) if found.iter().any(|f| f.name == m.name) => errors.push(format!("plugins/{file}: duplicate plugin name '{}'", m.name)),
            Ok(m) => found.push(m),
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    (found, errors)
}

/// `command` resolved against `dir` unless it is absolute.
pub fn command_path(dir: &Path, command: &str) -> PathBuf {
    dir.join(command)
}

/// Starts `m` in `dir` (its working folder) with `PLUGIN_ENV=1`, stdin and stdout piped, stderr inherited, and serves
/// MCP to it (`mcp::spawn_plugin`). On Windows without a console window.
pub fn launch(dir: &Path, m: &Manifest, ctx: &egui::Context) -> Result<Running, String> {
    let mut cmd = Command::new(command_path(dir, &m.command));
    cmd.args(&m.args).current_dir(dir).env(PLUGIN_ENV, "1").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: a console program started from the GUI must not open a console window.
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|err| err.to_string())?;
    let (Some(from), Some(to)) = (child.stdout.take(), child.stdin.take()) else {
        let _ = child.kill();
        return Err("no pipes to the plugin".into());
    };
    Ok(Running { name: m.name.clone(), child, bridge: crate::mcp::spawn_plugin(ctx.clone(), from, to) })
}

/// Starts `m` unless a plugin runs or there is no plugins folder; "Plugin <name> running" or
/// "Plugin <name> not started: <reason>" in the status line.
pub fn start(app: &mut CadApp, m: &Manifest, ctx: &egui::Context) {
    if app.plugin.is_some() {
        return;
    }
    let Some(dir) = app.plugins_dir.clone() else {
        app.status = format!("Plugin {} not started: no plugins folder", m.name);
        return;
    };
    match launch(&dir, m, ctx) {
        Ok(running) => {
            app.plugin = Some(running);
            app.status = format!("Plugin {} running", m.name);
        }
        Err(reason) => app.status = format!("Plugin {} not started: {reason}", m.name),
    }
}

/// Reads the plugins folder again into `app.plugins` and `app.plugin_errors`.
pub fn reload(app: &mut CadApp) {
    (app.plugins, app.plugin_errors) = app.plugins_dir.as_deref().map(scan).unwrap_or_default();
}

impl Running {
    /// The status line once the process has exited ("Plugin <name> finished", or "Plugin <name> failed: <exit status>");
    /// None while it runs.
    pub fn finished(&mut self) -> Option<String> {
        match self.child.try_wait() {
            Ok(None) => None,
            Ok(Some(s)) if s.success() => Some(format!("Plugin {} finished", self.name)),
            Ok(Some(s)) => Some(format!("Plugin {} failed: {s}", self.name)),
            Err(err) => Some(format!("Plugin {}: {err}", self.name)),
        }
    }
}

impl Drop for Running {
    /// Kills a plugin that still runs ("Stop plugin", app exit).
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
#[path = "plugins_tests.rs"]
mod tests;
