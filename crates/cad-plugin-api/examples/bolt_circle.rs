//! Example plugin: adds a bolt-circle flange to the document in one undo step.
//! From the Plugins menu (manifest: {"name":"Bolt circle","command":"<path>/bolt_circle.exe","args":["--count","8"],"version":"1"})
//! it talks to the running app over stdin/stdout. Standalone it starts the app itself and saves the result:
//! `bolt_circle --app target/debug/cad-app.exe --save flange.cadproj [--count 8] [--bolt-radius 40] ...`
//! It never prints to stdout: in a menu-started run stdout is the protocol.
use cad_plugin_api::{Client, bolt_circle};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(msg) => {
            eprintln!("{msg}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("bolt_circle: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<String, String> {
    let args = bolt_circle::parse_args(&std::env::args().skip(1).collect::<Vec<_>>())?;
    args.flange.check()?;
    let by_app = Client::launched_by_app();
    let mut client = if by_app {
        Client::stdio()
    } else {
        let app = args.app.as_deref().ok_or("--app <path to cad-app> is required when not started from the Plugins menu")?;
        if args.save.is_none() {
            return Err("--save <path> is required when not started from the Plugins menu".into());
        }
        Client::spawn(app)
    }
    .map_err(|err| err.to_string())?;
    let q = client.query().map_err(|err| err.to_string())?;
    let done = client.apply_commands(&args.flange.commands(&q.next, q.features)).map_err(|err| err.to_string())?;
    if !by_app && let Some(p) = &args.save {
        client.save_project(Some(p)).map_err(|err| err.to_string())?;
    }
    Ok(format!("bolt circle: {} holes, entities {}..{}, features {}..{}", args.flange.count, q.next.entity, done.next.entity - 1, q.next.feature, done.next.feature - 1))
}
