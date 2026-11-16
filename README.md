# ProjectCAD

A small CAD program written in Rust. Draw 2D sketches, make 3D models from them, save and export.

## Install

1. Install [Rust](https://rustup.rs) (stable, 2024 edition support needed).
2. Get the code and run:

```bash
git clone <this-repo-url>
cd ProjectCAD
cargo run --release -p cad-app
```

The first build takes a few minutes. No other tools needed.

## Use

**Draw (2D)**
1. Pick a tool on the left: Line, Circle, Rect, Arc, Polyline, Ellipse, Spline, Polygon, Text.
2. Click in the drawing area to place points. You can also type coordinates and press `Enter`.
3. `Esc` cancels the current tool.
4. Use Trim, Extend, Offset, Fillet, Chamfer to edit. Use Dimension, Leader, Radius dim, Angle dim to annotate.

**Make 3D**
- Use the Box, Cylinder, Sphere buttons, or build features from your sketches in the model panel.

**Move around**
- Scroll: zoom.
- Middle or right mouse drag: pan (2D) or orbit/pan (3D).
- `F`: fit everything on screen.

**Files**
- File menu: New, Open, Save, import/export (DXF, SVG, STL, OBJ).

## Shortcuts

| Key | Does |
|---|---|
| `Ctrl+N` / `Ctrl+O` / `Ctrl+S` | New / Open / Save |
| `Ctrl+Z` / `Ctrl+Y` | Undo / Redo |
| `Ctrl+D` | Duplicate |
| `Delete` | Delete selection |
| `F` | Fit to screen |
| `F3` | Toggle object snap |
| `F9` | Toggle grid snap |
| `Esc` | Cancel tool |

## Run the tests

```bash
cargo test --workspace
```
