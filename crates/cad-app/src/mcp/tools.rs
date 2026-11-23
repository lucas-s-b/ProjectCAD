use super::Request;
use eframe::egui;
use serde_json::{Value, json};

fn tool(name: &str, description: &str, props: Value, required: &[&str]) -> Value {
    json!({"name": name, "description": description,
        "inputSchema": {"type": "object", "properties": props, "required": required}})
}

pub fn definitions() -> Value {
    let num = json!({"type": "number"});
    let flag = json!({"type": "boolean"});
    let button = json!({"type": "string", "enum": ["primary", "secondary", "middle"], "default": "primary"});
    let world = json!({"type": "boolean", "default": false});
    let (shift, ctrl) = (json!({"type": "boolean", "default": false}), json!({"type": "boolean", "default": false}));
    json!([
        tool("screenshot", "PNG of the application window. Pixel coordinates of this image are the coordinates used by click, drag and scroll.", json!({}), &[]),
        tool("state", "JSON: active tool, view, status text, dirty flag, selection, current layer, 2D camera and viewport rectangle (pixels), window size, and the whole document (entities, layers), components, the bill of materials, mass properties of the bodies, components and assembly (mm, g, g·mm², with accuracy labels), the last motion study (joint values, traced point, first collision, with accuracy labels), joints with their loop flags and solved values (deg or mm, approximate (loop closed to 1e-9 mm)), the exploded view, the last interference check, the last analysis mesh (nodes, elements, quality, volume error, approximate (mesh)), the last static study (max displacement and von Mises with their nodes, reactions, out of balance, approximate (FEA, linear elastic, mesh h=…)), the safety factor (yield strength / max von Mises), the last validation report (benchmark rows with reference, result, error, tolerance and label), the timing of the last 3D scene build (perf: scene_ms, scene_vertices, instances, meshes), the feature dependency graph and recompute count of the last rebuild.", json!({}), &[]),
        tool("click", "Click. With world=true, x and y are millimetres in the 2D view instead of pixels.",
            json!({"x": num, "y": num, "button": button, "world": world, "shift": shift, "ctrl": ctrl}), &["x", "y"]),
        tool("drag", "Press at the first point, move, release at the second.",
            json!({"x1": num, "y1": num, "x2": num, "y2": num, "button": button, "world": world, "shift": shift, "ctrl": ctrl}),
            &["x1", "y1", "x2", "y2"]),
        tool("scroll", "Mouse wheel at a position; positive dy zooms in.",
            json!({"x": num, "y": num, "dy": num, "world": flag}), &["x", "y", "dy"]),
        tool("key", "Press and release one key.",
            json!({"key": {"type": "string", "description": "egui key name such as Enter, Escape, Delete, F3, Z"}, "shift": shift, "ctrl": ctrl}), &["key"]),
        tool("type_text", "Type text into the focused field.", json!({"text": {"type": "string"}}), &["text"]),
        tool("new_project", "Replace the document with an empty one. Unsaved changes are discarded.", json!({}), &[]),
        tool("open_project", "Load a .cadproj file. Unsaved changes are discarded.", json!({"path": {"type": "string"}}), &["path"]),
        tool("import_project", "Replace the document with the contents of a .dxf or .svg file (chosen by the extension). Unsaved changes are discarded.", json!({"path": {"type": "string"}}), &["path"]),
        tool("save_project", "Save to the path, or to the current file when omitted.", json!({"path": {"type": "string"}}), &[]),
        tool("export_project", "Export to .svg, .dxf or .pdf (visible 2D entities) or to .stl, .obj or .step (shown solid bodies and placed components), or .csv (bill of materials), chosen by the file extension.", json!({"path": {"type": "string"}}), &["path"]),
        tool("export_motion", "Write the last motion study as .csv: per step the joint value (deg or mm), the traced point (mm) and the first collision.", json!({"path": {"type": "string"}}), &["path"]),
        tool("widgets", "JSON array of the named widgets of the last frame: name, index (occurrence among equal names, draw order), x and y (centre) and w and h in screenshot pixels, visible.", json!({}), &[]),
        tool("click_widget", "Click the centre of the widget with this name and occurrence index (default 0). Scrolls it into view when needed.",
            json!({"name": {"type": "string"}, "index": {"type": "integer", "minimum": 0, "default": 0}, "shift": shift, "ctrl": ctrl}), &["name"]),
        tool("set_text", "Click a text field or drag value by name, select all, type the text and press Enter.",
            json!({"name": {"type": "string"}, "index": {"type": "integer", "minimum": 0, "default": 0}, "text": {"type": "string"}}), &["name", "text"]),
    ])
}

fn number(args: &Value, k: &str) -> Result<f32, String> {
    let v = args.get(k).and_then(Value::as_f64).ok_or(format!("missing or non-numeric argument '{k}'"))?;
    if v.is_finite() { Ok(v as f32) } else { Err(format!("argument '{k}' is not finite")) }
}

fn string<'a>(args: &'a Value, k: &str) -> Result<&'a str, String> {
    args.get(k).and_then(Value::as_str).ok_or(format!("missing or non-string argument '{k}'"))
}

fn flag(args: &Value, k: &str) -> Result<bool, String> {
    match args.get(k) {
        None | Some(Value::Null) => Ok(false),
        Some(v) => v.as_bool().ok_or(format!("argument '{k}' must be a boolean")),
    }
}

fn mods(args: &Value) -> Result<egui::Modifiers, String> {
    let ctrl = flag(args, "ctrl")?;
    Ok(egui::Modifiers { shift: flag(args, "shift")?, ctrl, command: ctrl, ..Default::default() })
}

fn index(args: &Value) -> Result<usize, String> {
    match args.get("index") {
        None | Some(Value::Null) => Ok(0),
        Some(v) => v.as_u64().map(|i| i as usize).ok_or("argument 'index' must be a non-negative integer".to_string()),
    }
}

fn button(args: &Value) -> Result<egui::PointerButton, String> {
    match args.get("button") {
        None | Some(Value::Null) => Ok(egui::PointerButton::Primary),
        Some(v) => match v.as_str() {
            Some("primary") => Ok(egui::PointerButton::Primary),
            Some("secondary") => Ok(egui::PointerButton::Secondary),
            Some("middle") => Ok(egui::PointerButton::Middle),
            _ => Err("button must be primary, secondary or middle".into()),
        },
    }
}

pub fn parse(name: &str, args: &Value) -> Result<Request, String> {
    Ok(match name {
        "screenshot" => Request::Screenshot,
        "state" => Request::State,
        "click" => Request::Click {
            x: number(args, "x")?,
            y: number(args, "y")?,
            button: button(args)?,
            world: flag(args, "world")?,
            mods: mods(args)?,
        },
        "drag" => Request::Drag {
            x1: number(args, "x1")?,
            y1: number(args, "y1")?,
            x2: number(args, "x2")?,
            y2: number(args, "y2")?,
            button: button(args)?,
            world: flag(args, "world")?,
            mods: mods(args)?,
        },
        "scroll" => Request::Scroll {
            x: number(args, "x")?,
            y: number(args, "y")?,
            dy: number(args, "dy")?,
            world: flag(args, "world")?,
        },
        "key" => {
            let k = string(args, "key")?;
            Request::Key { key: egui::Key::from_name(k).ok_or(format!("unknown key '{k}'"))?, mods: mods(args)? }
        }
        "widgets" => Request::Widgets,
        "click_widget" => Request::ClickWidget { name: string(args, "name")?.to_string(), index: index(args)?, mods: mods(args)? },
        "set_text" => Request::SetText { name: string(args, "name")?.to_string(), index: index(args)?, text: string(args, "text")?.to_string() },
        "type_text" => Request::Text(string(args, "text")?.to_string()),
        "new_project" => Request::New,
        "open_project" => Request::Open(string(args, "path")?.into()),
        "import_project" => Request::Import(string(args, "path")?.into()),
        "save_project" => Request::Save(match args.get("path") {
            None | Some(Value::Null) => None,
            Some(_) => Some(string(args, "path")?.into()),
        }),
        "export_project" => Request::Export(string(args, "path")?.into()),
        "export_motion" => Request::ExportMotion(string(args, "path")?.into()),
        _ => return Err(format!("unknown tool '{name}'")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_defaults() {
        let Ok(Request::Click { x, y, button, world, mods }) = parse("click", &json!({"x": 1, "y": 2.5})) else {
            panic!("not a click")
        };
        assert_eq!((x, y, world), (1.0, 2.5, false));
        assert_eq!(button, egui::PointerButton::Primary);
        assert!(!mods.shift && !mods.ctrl && !mods.command);
    }

    #[test]
    fn click_world_shift() {
        let Ok(Request::Click { world, mods, .. }) = parse("click", &json!({"x": 0, "y": 0, "world": true, "shift": true})) else {
            panic!("not a click")
        };
        assert!(world && mods.shift && !mods.ctrl);
    }

    #[test]
    fn ctrl_sets_command() {
        let Ok(Request::Key { key, mods }) = parse("key", &json!({"key": "Z", "ctrl": true})) else { panic!("not a key") };
        assert_eq!(key, egui::Key::Z);
        assert!(mods.ctrl && mods.command);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse("click", &json!({"x": 1})).unwrap_err().contains("'y'"));
        assert!(parse("key", &json!({"key": "NoSuchKey"})).unwrap_err().contains("unknown key"));
        assert!(parse("fly", &json!({})).unwrap_err().contains("unknown tool"));
        assert!(parse("click", &json!({"x": 1, "y": 1, "button": "left"})).is_err());
    }

    #[test]
    fn save_with_and_without_path() {
        assert!(matches!(parse("save_project", &json!({})), Ok(Request::Save(None))));
        assert!(matches!(parse("save_project", &json!({"path": "a.cadproj"})), Ok(Request::Save(Some(_)))));
        assert!(matches!(parse("export_motion", &json!({"path": "m.csv"})), Ok(Request::ExportMotion(_))));
    }
}
