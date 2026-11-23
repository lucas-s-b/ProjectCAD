//! Typed Rust client for ProjectCAD plugins. A plugin talks to the app over MCP (JSON-RPC 2.0, one message per line):
//! started from the app's Plugins menu it uses its own stdin/stdout (`Client::stdio`); standalone it starts
//! `cad-app --mcp --background` itself (`Client::spawn`). Document edits go through `Client::apply_commands`.
pub mod bolt_circle;
mod client;

pub use cad_document::{self, Command, FORMAT_VERSION, NextIds};
pub use cad_geometry;
pub use client::{Applied, Button, Client, PluginError, Pointer, Screenshot, Summary, ToolReply, Widget};
