# Technical Design (v0.1, Phase 1)

Status: living document. Evaluations below are desk evaluations from ecosystem knowledge as of mid-2026.
No comparative benchmarks have been run yet. Each choice marked "re-verify" must be benchmarked when its
subsystem is first built (rule 7 of the brief).

## 1. Technology decisions

| Subsystem | Chosen | Compared with | Reason | Adopted in |
|---|---|---|---|---|
| Language | Stable Rust, edition 2024 | nightly | No nightly feature is needed. | Phase 1 |
| GUI | egui / eframe | Iced, Slint | Pure Rust, MIT/Apache. Immediate mode suits tool-heavy CAD UI. First-class wgpu paint callbacks let the 3D viewport share one device. Slint has GPL/commercial licensing and its best renderer (Skia) is C++. Iced has weaker docking and custom-viewport support. | Phase 1 |
| GPU | wgpu + WGSL | ash (raw Vulkan), vulkano | Pure Rust (naga shader compiler), DX12/Vulkan/Metal from one code base, safe API. ash is Vulkan-only and unsafe. | Phase 1 |
| Math | glam (`DVec*` f64 for geometry, f32 for GPU) | nalgebra | Small, SIMD, plain types. nalgebra (or faer) is added only where general linear algebra is needed (constraint solver, FEA). | Phase 1 |
| Serialization | serde + serde_json | bincode, rkyv, postcard | Human-readable, diffable, easy to version and migrate while the model changes fast. Binary formats are evaluated for mesh caches after file-load profiling (re-verify; bincode maintenance status is doubtful). | Phase 1 |
| File dialogs | rfd | native per-OS code | Pure Rust on Windows; uses the XDG portal on Linux (no GTK needed). | Phase 1 |
| B-Rep kernel | Own: `cad-topology`, `cad-modeler` | truck, Fornjot, opencascade-rs | truck measured and rejected: C build through `vtkio`, booleans fail on coplanar and touching faces, no fillet (`docs/KERNEL_EVAL.md`). opencascade-rs is C++. | Phase 3 |
| Booleans | `manifold-rust` on the tessellation, face ids kept | boolmesh, csgrs, baby_shark, truck-shapeops | Passed all 11 measured cases with closed meshes; Apache-2.0; no C (`docs/BOOLEAN_EVAL.md`). boolmesh is the fallback. | Phase 3 |
| Polygon triangulation | `earcutr` | own ear clipping | Small, pure Rust port of a well-tested algorithm. | Phase 3 |
| Robust predicates | `robust` crate (Shewchuk port) | own implementation | Pure Rust, proven algorithm. | Phase 3 |
| Constraint solver | Own Levenberg-Marquardt on faer sparse normal equations (`cad-constraints`) | ezpz (KittyCAD) | ezpz exists (MIT, crate `kcl-ezpz`, 0.2.x) but is pre-1.0 and built around the KCL data model; we need degrees-of-freedom and redundancy reports over our own shapes. Not benchmarked against ours (re-verify). Sparse numeric Jacobian, sparse Cholesky (faer); rank from dense pivoted QR. | Phase 2 |
| Spatial index | Linear scan; rstar (2D snap) when needed; BVH over body triangles for 3D picks (Phase 3) | octree, k-d tree, grid | Measured: snap over 100 000 lines about 1 ms, pick about 11 ms (`docs/BENCHMARKS.md`). No index needed yet. | Phase 7 |
| Parallelism | rayon | scoped threads | Added with the first measured hot loop (tessellation). | Phase 3 |
| Collision / physics | parry, rapier | own | Pure Rust, mature. | Phase 5 |
| Text | egui built-in now; cosmic-text for drawing text | — | Dimension text needs shaping and outlines. | Phase 2 |

Rule check: every dependency must be pure Rust. `cargo tree -i cc`, `-i cmake`, `-i bindgen` must report
nothing for the Windows target. Licenses are listed with `cargo tree --format "{p} {l}"`.

## 2. Crates and dependency direction

```
cad-app ──► cad-document ──► cad-constraints ──► cad-geometry ──► glam
                  ├──► cad-params
                  └──► cad-modeler ──► cad-topology, cad-geometry, manifold-rust, earcutr
                  └──► cad-arch ──► cad-modeler, cad-geometry
   │
   └──────► cad-renderer ──► wgpu
```

- `cad-geometry`: shapes, tolerance, validation, tessellation, picking math. No UI, no GPU.
- `cad-document`: entities, commands, undo/redo history, native file format.
- `cad-arch` (Phase 4): wall joins, plan outlines with opening gaps, wall solids with opening cuts, door and window symbols, room areas, floor slab, roof and stair solids, IFC classes on a level. Works on shapes, not on the document; `cad-document` (rebuild) and `cad-app` (plan drawing) depend on it.
- `cad-renderer`: wgpu pipelines and WGSL. Knows only vertices and matrices; it does not depend on the kernel.
- `cad-app`: egui UI, cameras, tools, viewports.
- `cad-import-export` (added in Phase 2): `to_svg` and `to_dxf`. Depends on `cad-document` and `cad-geometry`;
  `cad-app` depends on it. The `dxf` and `roxmltree` crates are test-only dependencies used to read the output back.

The brief lists 23 crates. A crate is created when its first real code exists; empty crates are not created.
Planned splits: `cad-topology` and `cad-modeler` (Phase 3), `cad-constraints` and `cad-drafting` (Phase 2),
`cad-arch` (Phase 4, built), `cad-mechanical` and `cad-assembly` (Phase 5), `cad-analysis` (Phase 6),
`cad-import-export` and `cad-plugin-api` (Phase 7).

## 3. Geometry representation

- Model units are millimetres. Coordinates are `f64`. The GPU receives `f32`.
- Phase 1 shapes are analytic and parametric: `Line`, `Circle`, `Rect` (2D, on the XY plane) and `Box`,
  `Cylinder`, `Sphere` (3D). Each shape validates itself against a `Tolerance`. Invalid geometry is rejected
  with an error and never stored.
- Display geometry is derived: `tessellate()` gives triangles, `outline()` gives edge segments.
- Phase 3 adds B-Rep: geometry (curves, surfaces) and topology (vertex, edge, wire, face, shell, solid) in
  separate modules, joined by handles. Analytic shapes then become features that produce B-Rep bodies.

## 4. Document architecture

- A `Document` owns entities in an ordered map keyed by a stable `EntityId`.
- Every change is a `Command` (`Add`, `Remove`, `SetShape`). `Document::apply` validates, mutates and returns
  the inverse command. `History` stores inverse transactions, so undo and redo cost O(change), not O(document).
- A transaction is atomic: if one command fails, the applied ones are rolled back.
- `revision` counts changes. The app uses it for the unsaved-changes flag and to upload GPU buffers only when
  the scene changed.
- Native format `.cadproj`: JSON with a `version` field. Loading rejects newer versions and re-validates every
  shape. Saving writes a temporary file and renames it, so a crash cannot truncate a project.
- Phase 3 adds the feature dependency graph on top of the same command and history mechanism.

### Phase 2 additions
- Layers live in the document (`Layer`: name, colour, visible, locked). Layer 0 always exists. Entities carry a
  `LayerId`. Layer edits are commands, so they undo and mark the project dirty. Hidden layers are not drawn or
  picked; locked layers are drawn but not picked.
- `History::execute_merged` folds a run of "set" commands with the same key into one undo step (colour drags).
- Format version 2 adds layers. Version 1 files load through serde defaults; there is no per-version code path.
- Snapping is pure geometry in `cad-geometry::snap`: snap points per shape plus pairwise intersections of
  segment and arc primitives. Candidates are filtered by bounding box with a linear scan.
- 2D transforms are similarity matrices (`DAffine2`). A rectangle rotated off-axis becomes a closed polyline.
  Solids reject 2D transforms until Phase 3 gives them a full placement.

### Constraints
- `cad-constraints` holds the constraint types and the solver. It works on plain shapes and knows no document.
- Variables: Line `[a, b]`, Circle `[centre, r]`, Arc `[centre, r, start, sweep]`. Each constraint gives 1 or 2
  equations; the solver minimises them with Levenberg-Marquardt, which also keeps movement small when the sketch
  is under-constrained.
- After solving, the rank of the Jacobian gives the remaining degrees of freedom and the redundant equations.
  A set that does not converge is reported as unsatisfiable and nothing is changed.
- The document stores constraints. `History::execute` solves after the user's commands and records both in one
  undo step; if solving fails, the whole change is rolled back. Undo and redo replay stored commands and do not solve.
- An entity referenced by a constraint cannot be removed until its constraints are removed in the same transaction.

## 5. Rendering

- 2D viewport: egui painter, vector lines, top view of the XY plane.
- 3D viewport: wgpu inside an egui paint callback, depth buffer, one WGSL shader for lit triangles and unlit
  lines. Vertex buffers persist on the GPU and are replaced only when the scene revision changes.
- Later: instancing, batching, frustum culling, LOD, hidden-line rendering (Phase 7 targets after baselines).

## 6. Missing capabilities that must be built natively

Robust booleans, fillets, shelling and offsets; the sketch constraint solver; complete STEP and IGES; IFC;
FEA solvers; hidden-line removal. DWG has no acceptable pure-Rust path and stays a separate investigation.

## 7. Known Phase 1 limits

- Selection scans every entity. Acceptable for thousands of entities; an index comes with the Phase 2 benchmarks.
- No line types, line weights or per-entity colour yet (Phase 2).
- Units change only how lengths are displayed (dimensions, status bar). The model and the Properties fields stay in millimetres.
- Text width is estimated (0.6 x height per character) until real text shaping is added. The 3D view shows text
  and dimensions as outlines. Export is 2D only: solids are skipped, dimensions are exploded in DXF.
- Entities move in the 2D view, by numeric input, or (solids) with the translate gizmo of the 3D view.

## 8. MCP mode (test and automation interface)

`cad-app --mcp` runs the normal GUI and also serves the Model Context Protocol on stdin/stdout
(`crates/cad-app/src/mcp/`). A client can drive the real application without operating-system input:

- `click`, `drag`, `scroll`, `key`, `type_text` inject events into egui (`App::raw_input_hook`). Positions are
  window pixels, or 2D world millimetres with `world: true`.
- `state` returns the tool, view, selection, cameras and the whole document as JSON.
- `screenshot` returns the application's own framebuffer as PNG.
- `new_project`, `open_project`, `save_project` replace the native file dialogs, which are disabled in this mode.

The protocol is hand-written JSON-RPC over newline-delimited JSON; no async runtime and no MCP library.
`.mcp.json` registers the debug build as the `projectcad` server. `tests/mcp_smoke.sh` is the end-to-end test.
The window must not be minimized, or requests time out after 30 s. In this mode the window ignores the real
mouse and keyboard: only MCP input reaches it, so a person moving the mouse over it cannot disturb a test.

Widgets are registered by name every frame (`crates/cad-app/src/widgets.rs`): buttons, labels and checkboxes by
their text, fields and combo boxes by their caption, repeated rows by occurrence index. `click_widget` and
`set_text` use these names and scroll a widget into view first; `widgets` lists them. Test scripts use names for
all UI clicks, so adding rows does not break them. Picks inside the 3D viewport still use pixel positions.

## 9. Solids, parameters and features (Phase 3)

- A 2D `Profile` (outer loop and holes of line and arc segments) is the input of every sweep.
- `cad-topology::Brep` is one closed shell: vertices, edges (line or arc), faces (analytic surface, outward flag,
  loops of coedges). `validate` checks manifoldness, loop closure and that vertices lie on their surfaces.
- `cad-modeler` builds B-Reps by `extrude` and `revolve` and tessellates them watertight: every edge is sampled
  once and shared; planar faces are triangulated, curved faces are four-sided grids projected onto their surface.
- Sweep, loft and the tapered extrude build their mesh directly: rings of profile samples joined by ruled quads, caps triangulated. A tapered extrude moves every loop to its material side by |distance| · tan(angle) with mitred vertices and refuses moves whose edges turn around or cross; straight walls keep a plane surface, walls of arcs are faceted; a zero taper uses the B-Rep extrude. A smooth loft runs every column of ring points through a uniform Catmull-Rom curve (mirrored end points, 8 rings per span).
- A `Body` is the tessellation plus, for every triangle, the analytic surface it lies on. Booleans combine bodies
  through `manifold-rust` and keep that mapping through its face ids. `TrimmedBody::from_body` rebuilds faces on planes and cylinders: one face per connected group of triangles, boundary loops walked on the mesh and split into the body edges, vertices snapped onto all their surfaces (1e-6). STEP writes such bodies as advanced B-Reps and falls back to a faceted B-Rep per body otherwise.
- `cad-params` evaluates named expressions with units; a bare number means millimetres or degrees by context.
- The document stores parameters and an ordered feature list. `Rebuilder` evaluates the list into one body. Each
  feature's result is cached under a key hashed from its resolved inputs and the previous feature's key, so
  unchanged features are not recomputed. A failed or suppressed feature leaves the body as it was.
- Entities used by a feature cannot be removed while the feature exists.
- Every entity belongs to a sketch; a sketch has a plane: a principal plane with an offset expression, or a plane
  on a picked face. The 2D view edits the active sketch only. Features take their plane from
  the sketch of their profile.
- The rebuild keeps a list of bodies. A body's id is the id of the feature that created it. `Add`, `Cut` and
  `Intersect` act on a target set (automatic, all bodies, or one body); `New` starts a body.
- Body edges are found on the mesh as boundaries between faces of different surfaces, after faces on the same
  surface are merged. A fillet or chamfer builds a tool solid along the edge (extruded for a straight edge,
  revolved for a circular rim) whose cross-section is the corner outside the fillet arc or the chamfer triangle,
  then subtracts it on a convex edge or adds it on a concave edge. The tool is tessellated finer than the body.
- Picks are references, not snapshots: a sketch on a face stores the plane, the picked point and the body; a fillet or chamfer edge stores a point and the keys of its two faces (outward plane normal or cylinder axis). The rebuild places each face sketch once, before the first feature that uses it (unused ones on the final bodies), on the planar face with the same normal under the picked point, nearest to it; edges are the nearest edge between faces with matching keys. Placed planes are returned in `RebuildResult::planes` and shown by the app.

## 10. Architecture (Phase 4)

- Architectural elements are entities with their own `Shape` variant (`Wall`, `Opening` for doors and windows, `Room`, `Slab`, `Roof`, `Stair`), so selection, transforms, undo, layers, snapping and the file format come from the entity code.
- A level is a sketch on XY with `level: Some(Level { height })`: name = storey name, offset expression = elevation, height = floor-to-floor height. Walls, rooms, floors, roofs and stairs live only on levels (`NotALevel`).
- Joins (`cad_arch::footprints`): ends within 0.001 mm meet; around the meeting point the ends are sorted by angle and each pair of neighbours meets where their facing faces cross. Three or more ends add the meeting point to each outline, so the hub is covered once. An end on another wall's baseline meets that wall's faces (T); crossing walls stay whole (X). Corners farther than 5 x the two thicknesses fall back to square ends.
- Plan (`cad_arch::plan`): every outline is filled; an outline edge is drawn only where no other outline covers it, which cleans L, T and X joins without a polygon boolean.
- Solids: the rebuild extrudes each outline from the level plane by the wall height into `RebuildResult::walls`. Walls are not features and are recomputed on every rebuild.
- Openings: `Shape::Opening` stores its host wall's entity id, the distance `at` along the baseline to its centre, width, height, sill, kind (Door/Window) and the door's hinge and swing sides, plus a copy of the host wall that `History::execute` refreshes after every edit (like linked dimensions), so bounds and picking need no lookup. The document refuses hosts that are not walls on the same level (`NotAHost`) and removing a wall that hosts openings (`HostInUse`); the app deletes them with it. A shortened wall slides its openings back inside without changing `at`.
- Cuts: the rebuild subtracts one box per opening from its wall solid (manifold difference), one thickness past each face, and past the floor, the top or a wall end the opening reaches, so no faces are coplanar. The plan clips each footprint into pieces beside the gaps before filling and edge cleaning.
- Rooms: `Shape::Room` stores a seed point, name and number; it is placed by one click inside an enclosed area. The seed carries the name through wall edits, which a purely computed room could not. `cad_arch::regions` chains the cleaned plan outline (without opening gaps) into loops; wall is on the left of every edge, so clockwise loops are enclosed areas. A room is the smallest one around its seed minus the free-standing wall groups inside it; net area and perimeter go into `RebuildResult::rooms`.
- Floors: `Shape::Slab` stores a closed outline and a thickness; the rebuild extrudes it from the thickness below the level plane up to it, into `RebuildResult::walls` with the wall solids. "Make floor" copies a room's current area or replaces a selected closed curve.
- Roofs: `Shape::Roof` stores the eave outline, kind (Flat, Shed, Gable, Hip), pitch, thickness at right angles to the slope and overhang. Every sloped plane rises at the same pitch from its eave line, so in plan a plane is the part of the eaves outline where its eave line is the nearest sloped one (`Roof::pieces`, half-plane clipping); on a convex outline that is the straight skeleton, so gable and hip need one. The solid (`cad_arch::roof_body`) is, per plane, a tall prism over its piece intersected with a sheet on the sloped plane; the planes only touch and are joined into one body without a union. The underside meets the eave line on top of the level.
- Stairs: `Shape::Stair` stores start, end of the run, width, height and riser count; the tread depth is the run over the treads (one fewer than the risers), so transforms work on the two points. The Stair tool takes the riser count from the storey height and the maximum riser. The solid (`cad_arch::stair_body`) is the stepped side profile, filled to the floor, extruded across the width.
- Multi-floor: the 3D view shows every level at its elevation; the plan can draw the level below or all other levels in grey under the active one. `Document::level_change` stores an edited level and gives a new storey height to the walls and stairs that still have the old one, in one undo step.
- BIM: the entity `meta` map is the property bag (`Pset_X.Prop` keys become IFC property sets). IFC class from the shape kind, storey from the level, quantities computed from the solids and GlobalIds derived at export. Doors and windows export as IfcDoor/IfcWindow (`cad_arch::ifc_class`) with an IfcOpeningElement that voids the host wall. Rooms export as IfcSpace (number as Name, name as LongName, NetFloorArea and NetPerimeter from the region) and floors as IfcSlab. Roofs export as IfcRoof and stairs as IfcStair. IFC export itself is deferred to Phase 7.
