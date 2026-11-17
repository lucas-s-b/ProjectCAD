# Roadmap

Each phase ends with a buildable app, passing tests and an updated `DESIGN.md`. Nothing is marked done
until it works in the running app.

## Phase 1: Core foundation (done; every item checked in the running app through MCP)
- [x] Workspace: `cad-geometry`, `cad-document`, `cad-renderer`, `cad-app`
- [x] Shapes: line, circle, rectangle, box, cylinder, sphere; tolerance and validation
- [x] Document, commands, atomic undo/redo
- [x] Native `.cadproj` format: new, open, save, save as
- [x] 2D viewport: pan, zoom, fit, grid
- [x] 3D viewport (wgpu): orbit, pan, zoom, fit, shaded solids, edges
- [x] Selection in both viewports, move, numeric edit, delete
- [x] Draw tools: line, circle, rectangle; add box, cylinder, sphere
- [x] Tests: geometry, document, file round trip, camera, shader validation

## Phase 2: Advanced 2D CAD (core done; leftovers listed below)
Milestone 1 (done; checked in the running app through MCP):
- [x] Layers: colour, visibility, lock, current layer, add; file format version 2 (version 1 still loads)
- [x] Layers: rename, delete, colour edit with merged undo, entity layer change
- [x] Snapping: endpoint, midpoint, centre, intersection
- [x] Grid snap, Shift polar constraint, snap while moving
- [x] Entities: arc, polyline
- [x] Rotate, mirror, duplicate; scale

Milestone 2 (built; each tool checked in the running app through the MCP mode on lines, other curve types unit-tested):
- [x] Trim (line, arc, circle, polyline, rectangle) against any visible curve
- [x] Extend (line, arc, open polyline) to the first boundary
- [x] Offset (line, arc, circle, rectangle, polyline with mitre joins and no self-intersection cleanup)
- [x] Fillet and chamfer between two lines

Milestone 3 (done; tools and export checked in the running app through MCP):
- [x] Text entity and aligned linear dimension (2D view draws them fully; the 3D view shows outlines only)
- [x] Units (mm, cm, m, in, ft) and decimal precision for displayed lengths; file format version 3
- [x] SVG export and DXF R12 export (`cad-import-export`); DXF read back with the `dxf` crate in tests
- [ ] Units and decimals controls, File > Export menu items (built, not exercised in the app)
- [ ] Exported DXF opened in an external CAD program; exported SVG rendered in a viewer

Milestone 4 (done; solver unit-tested, UI checked in the running app through MCP):
- [x] `cad-constraints`: 15 constraint types on lines, circles and arcs; Levenberg-Marquardt solver
- [x] Under-, fully and over-constrained reporting (degrees of freedom, redundant equations); inconsistent sets are rejected
- [x] Document solves after every change inside one undo step; file format version 4
- [x] Constraint buttons by selection, constraints list, driving values (edit a Distance and the geometry follows)
- [ ] Trim, extend, fillet on constrained entities; undo while a value edit is pending (not exercised)

Milestone 5 (done; checked in the running app through MCP unless noted):
- [x] Layer line types (continuous, dashed, hidden, center) and line weights in the 2D view, SVG and DXF; file format version 5
- [x] Ellipse and spline entities; polygon tool
- [x] Perpendicular and tangent snap
- [x] Typed coordinates: `x,y`, `@dx,dy`, `@d<a`
- [x] Rectangular array (checked) and polar array (unit-tested only)

Milestone 6 (done; checked in the running app through MCP unless noted):
- [x] Hatch (parallel lines in a closed boundary), leader, radial and angular dimensions
- [x] DXF import with a skip report (`import_project`); round trip through our own export checked
- [x] Benchmark baseline: `docs/BENCHMARKS.md` (`cargo run --release -p cad-bench`)
- [ ] File menu items with native dialogs (Export, Import): cannot be driven through MCP, not exercised
- [ ] DXF opened in an external CAD program; SVG rendered in a viewer

What the baseline says (see `docs/BENCHMARKS.md`):
- Snap over 100 000 lines takes about 1 ms and picking about 11 ms: no spatial index is needed yet.
- The solver takes about 15 ms for a 150-line chain after the move to sparse normal equations (Phase 3).
- A 100 000-line project is 31 MB of JSON and saves in about 80 ms: a binary format is not urgent.

Leftovers batch (done; unit-tested, main paths checked in the running app through MCP):
- [x] Construction geometry; split, join, align, distribute
- [x] Trim, split and intersection snap for ellipse and spline (through a 1e-4 mm flattening, not exact)
- [x] Constraints: point on line, point on circle, Symmetric button, glyphs in the 2D view, pinned dragging
- [x] Groups and object metadata; associative linear dimensions; hatch patterns (lines, cross, solid)
- [x] SVG import; PDF export (hand-written vector PDF); file format version 11
- [ ] Not exercised in the app: hatch pattern combo, Unlink, metadata remove, group selection in 3D and in the Objects list, File menu import/export dialogs
- [ ] PDF never opened in a viewer; DXF, SVG, STL never opened in another program

Still open from the brief's 2D scope:
1. Blocks with shared definitions and instances
2. Infinite construction lines, stretch, transforms with a picked base point
3. Extend and offset for ellipse and spline; exact conic and spline intersections; fillet with arcs; hatch islands
4. Constraints on rectangle and polyline
5. Associative radial and angular dimensions and hatches; true DXF DIMENSION entities
6. Parallel snap, user coordinate systems, line types and annotation text in the 3D view
7. Speed of ellipse and spline snapping (about 2000 segments per ellipse on every mouse move; not measured)

## Phase 3: Geometry and parametric modeling (in progress)
Kernel decisions, both from measured spikes: truck rejected (`docs/KERNEL_EVAL.md`), own B-Rep kernel;
`manifold-rust` chosen for booleans (`docs/BOOLEAN_EVAL.md`).

Done (unit-tested; the plate-with-hole flow checked in the running app through MCP):
- [x] `cad-params`: parameter expressions with units, functions, conditionals, dependency order, cycle detection
- [x] `cad-geometry::profile`: closed profiles with holes from lines, arcs, circles, rectangles, polylines
- [x] `cad-topology`: B-Rep (vertex, edge, coedge, loop, face; plane, cylinder, cone, sphere, torus) with validation
- [x] `cad-modeler`: extrude, revolve, watertight tessellation, volume, area, centroid
- [x] Bodies and booleans (union, difference, intersection) with the analytic surface kept per triangle
- [x] Feature history in the document: extrude and revolve with add / cut / intersect, suppress, reorder, edit, undo
- [x] Incremental rebuild: only features whose inputs changed are recomputed; a failed feature does not stop the rest
- [x] Model tab: parameters, features, body properties; the body in the 3D view
- [x] Sketches on XY, XZ, YZ planes with offset expressions; sketch on a picked planar face (follows the face on rebuild since file format 13)
- [x] Several bodies: New body, targets (auto, all, one body), flip for cuts into the material; file format version 7
- [x] 2D view per active sketch with the bodies as grey reference edges; sketches placed on their planes in 3D
- [x] Faces on the same surface merged; body edges (line, circle, other; convex, concave, smooth)
- [x] Fillet and chamfer features on straight edges and on circular rims, made with booleans; edges picked in the 3D view
- [x] STL (binary) and OBJ export of the bodies, units mm; files read back in tests and compared by volume
- [x] Sweep along line and arc paths (mitred corners, open or closed) and ruled loft through sections; a `Faceted` surface kind for free-form faces; file format version 9
- [x] Shell feature: hollow a body, optionally open on a picked planar face (Minkowski erosion in manifold-rust); file format version 10
- [x] Rebuild runs on a worker thread; the window stays responsive and shows "Rebuilding..."
- [x] Linear pattern, circular pattern and mirror features (of a feature or of all bodies); pattern of a pattern gives a grid; count is an expression; file format version 8
- [x] Rebuild worker: a running rebuild stops when a newer request or a reset arrives (checked between features); a panic in a feature is reported as that feature's error, a panic outside a feature as an error on all features
- [x] Sparse constraint solver (faer sparse Cholesky on the normal equations); a 400-line chain takes 178 ms (was 3.1 s)
- [x] STEP export of the bodies as faceted B-Reps (AP214, one planar face per triangle), units mm; read back in tests and compared by volume
- [x] Trimmed B-Rep faces rebuilt from the tessellation for planes and cylinders (boundary loops, snapped vertices); plane-cylinder edges are exact lines, circles or ellipses
- [x] STEP export with analytic faces: ADVANCED_FACEs on PLANE and CYLINDRICAL_SURFACE with LINE, CIRCLE and ELLIPSE edges; other bodies are written faceted in the same file
- [x] Face and edge references: a sketch on a face and fillet/chamfer edges keep which way their faces point and are found again on every rebuild, so they follow when earlier features move them; file format version 13
- [x] BVH over body triangles for 3D ray picks (cached per body)
- [x] Face selection in the 3D view: click a body face to highlight it and show its surface kind and area
- [x] Section plane in the 3D view (axis, offset, flip); display only, picks still hit hidden parts
- [x] Section caps: cut faces of feature bodies filled in the section view (not on Box/Cylinder/Sphere primitives)
- [x] Edge selection in the 3D view: click an edge to see its kind, length or radius and convexity; hidden edges are not picked (blend picking too)
- [x] Translate gizmo in the 3D view for selected Box, Cylinder and Sphere entities (drag an axis arrow); no grid snap
- [ ] Built, not exercised in the app: Circular pattern and Mirror feature buttons, Revolve button, Intersect, Target other than Auto, reorder and delete buttons, feature editor fields other than distance, parameter and sketch delete, non-planar face pick, Re-pick edges, the highlight of picked edges

Limits of this slice (state them wherever results are shown):
- Booleans run on the tessellation (0.02 mm): curved faces take part as facets, intersection curves are polylines.
  Volumes of curved bodies are approximate and labelled so in the UI.
- A sketch on a face follows the face along its normal (the picked point must still fall on a face pointing the same way); it does not follow a face that tilts. Sketches from files before version 13 keep the plane as picked.
- The feature history is linear: changing an early feature recomputes everything after it.
- Ellipse and spline cannot be part of a profile. Crossing profile loops are not detected.
- STEP: a body with any face other than a plane or cylinder, or an edge other than a line, circle or ellipse, is exported faceted.
- Shell: the offset tool is a faceted sphere, so walls next to curved or slanted faces are slightly thicker than asked; an open shell needs a thickness of at most about a quarter of the body depth along the opening direction.
- Fillets and chamfers: no blending at corners where filleted edges meet; a stored edge is found again as the nearest edge between faces pointing the same ways as when picked; if the picked edge is gone, another such edge may be blended. Edges from files before version 13 are picked points and report "edge not found" when moved. A rim fillet on a round hole carries a
  few percent volume error at the 0.02 mm body tolerance (mesh slivers between facets and the true circle).

Remaining:
1. A dependency graph between sketches, features and bodies instead of the linear history; references for shell faces and for faces that tilt (face and edge references that follow moved faces done)
2. Trimmed faces and analytic curves beyond plane and cylinder (cones, spheres, tori, cylinder-cylinder); booleans still run on the mesh
3. Smooth loft through three or more sections, loft sections with holes, self-intersection checks, draft; corner blending where filleted edges meet; fillets on other edge types
4. NURBS curves and surfaces
5. (done) Constraint values from parameter expressions; solver Jacobian restricted per variable. Sparse normal equations (faer); a 400-line chain now takes 178 ms
6. (done) 3D gizmo, edge selection, section caps
7. STEP export of cones, spheres, tori and free-form faces (planes and cylinders done)
8. (done) Handle a rebuild worker panic; cancel a running rebuild when a newer request arrives

## Phase 4: Architectural CAD
Walls with automatic joins, doors, windows, floors, roofs, stairs, room detection and areas,
multi-floor projects, BIM metadata designed for later IFC export.

## Phase 5: Mechanical CAD
Parametric generators (gears, shafts, bearings, springs, fasteners, threads), assemblies, joints,
collision and interference checks (parry), exploded views, bill of materials.

## Phase 6: Advanced engineering
Mass properties, kinematics, static structural analysis, FEA infrastructure. Every result is labelled
approximate or validated; unvalidated results are never presented as certified.

## Phase 7: Optimization and production
Profiling-driven work: instancing, culling, LOD, mesh caches, binary cache format, parallel tessellation.
STEP, IGES, STL, OBJ, 3MF, glTF, PDF, IFC. Plugin API. Docking, themes, command search. Documentation.
