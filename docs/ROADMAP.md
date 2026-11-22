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

## Phase 3: Geometry and parametric modeling (done; deferred items listed below)
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
- [x] Tapered extrude: a draft angle (an expression) leans the side walls about the extrusion's start plane, holes widen as the walls narrow; smooth loft through three or more sections (Catmull-Rom, an option per loft); file format version 14
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
- Tapered extrude: walls of arcs and circles are faceted (no analytic cone), so STEP writes them faceted; a draft that turns an edge around or makes the moved outlines cross is refused; collinear overlaps of the moved outlines are not detected.
- Smooth loft: uniform Catmull-Rom through the section points, 8 rings between two sections, no self-intersection check; with two sections it is the ruled loft.

Deferred to later phases:
- Dependency graph between sketches, features and bodies instead of the linear history; references for shell faces and for faces that tilt: Phase 5 (assemblies and part generators need stable references)
- Draft of existing faces about a neutral plane (any body, not only at extrude time): Phase 5 batch F (the faces must be named by stable face references); corner blending where filleted edges meet, fillets on other edge types: Phase 7 (vertex blends need free-form surfaces)
- Loft sections with holes; self-intersection checks for sweep and loft: Phase 7
- Trimmed faces and analytic curves beyond plane and cylinder (cones, spheres, tori, cylinder-cylinder); booleans on exact geometry instead of the mesh: Phase 7
- NURBS curves and surfaces: Phase 7
- STEP export of cones, spheres, tori and free-form faces (planes and cylinders done): Phase 7. Phase 5 parts are mesh-boolean solids checked with parry on meshes, so they do not need it first.

## Phase 4: Architectural CAD (done; IFC export and other deferred items listed below)
Walls with automatic joins, doors, windows, floors, roofs, stairs, room detection and areas,
multi-floor projects, BIM metadata designed for later IFC export.

Done (unit-tested; levels, walls, joins, doors, windows, rooms, floors, roofs, stairs, level views and their edits checked in the running app through MCP):
- [x] Levels: a sketch on XY with a storey height; its offset expression is the elevation; several levels make a multi-floor project; file format version 15 (older files load without levels)
- [x] Straight walls (baseline, thickness, height, centre/left/right alignment) drawn with the Wall tool on a level, edited in Properties, moved, copied and mirrored like other entities
- [x] Automatic joins (`cad-arch`): L mitres, T ends stop at the through wall's face, X crossings, three or more walls at a point share the hub
- [x] Plan view: wall outlines filled, join lines hidden; 3D: one extruded solid per wall on its level
- [x] Doors and windows: one click inside a wall with the Door or Window tool; width, height, sill, position along the wall, hinge side and swing side in Properties; file format version 16
- [x] Openings are hosted by their wall (entity id + distance along the baseline): they follow it when it moves, is edited or mirrored, slide back inside when it is shortened, are copied with it, and are deleted with it
- [x] Each opening cuts a box from its wall solid (manifold boolean) and a gap from the plan; doors show a leaf and swing arc, windows the face and glass lines; IFC classes IfcDoor/IfcWindow from the kind
- [x] Rooms: one click inside an area enclosed by walls with the Room tool; the area is found from the cleaned wall outline on every rebuild (doors do not break it, free-standing walls are subtracted), so names stay when walls move; net area and perimeter; tag in the plan; Rooms list with name and number edited in place; IfcSpace
- [x] Floors (slabs): "Make floor" in Properties from a room's area or a drawn closed outline (the outline becomes the floor), with a thickness; a solid below the level in 3D; IfcSlab; file format version 17
- [x] Roofs: "Make roof" in Properties from a room's area or a drawn closed outline; flat, shed (rises from the outline's first edge), gable (from the longest edge and the edges parallel to it) and hip (from every edge); gable and hip need a convex outline, where the roof planes are its straight skeleton; pitch, thickness at right angles to the slope, overhang; sits on top of its level; roof planes drawn in the plan; IfcRoof
- [x] Stairs: straight flight with the Stair tool (start point, direction); as many risers as the level height needs at the maximum riser, treads of the set depth; sides, riser lines and walking arrow in the plan, a stepped solid in 3D; IfcStair; file format version 18
- [x] Multi-floor: the plan shows the active level alone, with the level below, or with all levels, the others in grey; the 3D view shows every level at its elevation; "Apply to level" gives a new storey height to the walls and stairs that still have the old one (one undo step)

Limits of this slice:
- Walls and stairs copy the storey height when drawn; "Apply to level" updates those that still have the old height, but a change of a parameter used in the height expression does not, and no wall attaches to the level above.
- Ends join only within 0.001 mm of each other or of another wall's baseline; joins sharper than about 6 degrees get square ends.
- 3D wall solids are separate per wall (join lines show as edges), not pickable above the floor, and get no section caps.
- The plan shows the whole wall outline; there is no cut height yet.
- Openings cannot be dragged along their wall (edit "at"), have no placement preview, no 3D leaf, frame or glass, and are not exported to DXF/SVG.
- An opening is a straight box through its wall; across a join or in a construction wall it is not cut or drawn.
- A floor made from a room is a copy of its outline: it does not follow later wall edits. Slab outline points are not editable one by one.
- Room tags are drawn at a fixed screen size, show areas in the display unit squared, and are not exported to DXF/SVG; a room's new number is the highest numeric room number on the level plus one.

- Roofs: gable and hip only on convex outlines (no straight skeleton for concave ones); the planes are separate solids in one body, joined without a union; walls are not trimmed to the roof; a roof made from a room starts at the inner wall faces unless the overhang adds the wall thickness; "Make roof" or "Make floor" from a drawn curve replaces the curve, so make the other one from a room or a new curve.
- Stairs: straight flights only, no landings, turns, railings or opening in the slab above; the solid is filled down to the floor; a new height keeps the riser count.
- Level views are 2D only and not exported; other levels are neither pickable nor snappable.

Deferred to later phases:
- IFC export: storeys, walls, openings, spaces, slabs, roofs, stairs, property sets from entity metadata (the design above: classes from `cad_arch::ifc_class`, quantities from the solids, GlobalIds derived at export): Phase 7 (export formats)
- Roofs on concave outlines (straight skeleton), dormers, roof openings, walls trimmed to the roof: Phase 7
- Stairs with landings, turns or spirals, railings, the stair opening in the slab above: Phase 7
- Plan cut height, walls attached to the level above, openings in slabs, dragging openings along their wall: Phase 7

## Phase 5: Mechanical CAD (in progress)
Parametric generators (gears, shafts, bearings, springs, fasteners, threads), assemblies, joints,
collision and interference checks (parry), exploded views, bill of materials.

Done (unit-tested; part generators, components, the bill of materials, joints, exploded views, the interference check, bearings and modelled threads checked in the running app through MCP):
- [x] Part features: gear, shaft, spring, bolt and nut generators in the feature history ("Add part" in the Model tab), placed on a sketch plane at an (x, y) point with the axis along the plane normal; every parameter is an expression; a part makes a new body by default and can add, cut or intersect; patterns and mirrors copy parts; file format version 19
- [x] Involute spur gear: module, teeth, pressure angle, face width, bore; no profile shift or backlash, addendum 1 m, dedendum 1.25 m; flanks in 10 lines, tip and root as arcs
- [x] Stepped shaft: diameter and length per step, 45 degree chamfer at both ends (`cad_modeler::lathe`, a turned mesh with exact plane, cylinder and cone faces)
- [x] Helical compression spring: wire and mean diameter, pitch, turns; plain ends
- [x] ISO 4017 hex head screw and ISO 4032 hex nut, M3 to M24 coarse; cosmetic thread (nominal diameter, thread named only)
- [x] Bill-of-materials data per part: designation ("ISO 4017 M8x30", "Spur gear m2 z20 b10") and thread derived from the parameters, shown in the feature editor
- [x] Components (assemblies inside one document): placed instances of bodies, one body any number of times; translation and turns about X, Y and Z as expressions; a body with components is drawn only through them; Assembly tab (add from a body, list, select in the list or in 3D, name, position, rotation, material, part number, delete); the translate gizmo moves the selected component; a placed body cannot be removed; file format version 20
- [x] Bill of materials: rows grouped by designation (by body when there is none), material and part number; item, part name, designation, quantity, mass from a density table; shown in the Assembly tab, exported as CSV (File > Export BOM CSV..., `export_project` with `.csv`), in the MCP `state` (`bom`)
- [x] Joints between components: fixed, revolute (angle) and slider (distance), values as expressions (driving a joint or a parameter moves the child and everything jointed to it); a frame (origin and axis) on each component, typed or taken from a clicked flat or cylindrical face; placed by walking the joint tree from the roots, exact, no solver; a second parent, a loop or a grounded child is refused; Assembly tab "Joints" section; file format version 21
- [x] Grounded components (never a joint child; the gizmo does not move them)
- [x] Exploded views: steps that move one component along a direction by a distance, run one after another; on/off toggle, explode slider and factor, trail lines; display only (BOM, state placements and export stay assembled)
- [x] STL/OBJ/STEP export writes the shown bodies and the placed components (joints applied), each solid named (`Body_<id>` or the component name)
- [x] Interference and clearance check between placed components, on demand ("Check" in the Assembly tab): pairs whose bounding boxes are near are measured with `parry3d-f64` first and only pairs whose surfaces touch or whose boxes nest are intersected with the manifold boolean (exact common volume, the common solid drawn red with its components as edges); touching faces and slivers thinner than 0.04 mm count as contact, so a bolt in its nut (cosmetic thread) is contact; minimum distance up to a clearance limit with `parry3d-f64`, the closest points drawn as a line; results in the MCP `state` (`check`), dropped by any edit
- [x] Deep groove ball bearings (ISO 15, 6000 to 6010 and 6200 to 6210) as a part generator: inner and outer ring with ball grooves and a full set of balls, internal geometry derived from bore, outside diameter and width (ball diameter 0.3 (D - d), groove radius 0.52 of the ball); designation "Ball bearing 6204 20x47x14"; file format version 22
- [x] Modelled ISO metric threads as an option on bolts and nuts ("Modelled thread"): ISO 68-1 basic profile on a helical lattice mesh (`cad_modeler::screw`, 48 facets per turn, profile corners exact), bolt thread 0.04 mm under the basic profile, nut thread on it; a nut fits when it sits a whole number of pitches from the bolt's bearing face (or is turned to match), otherwise the interference check shows the flank overlap; cosmetic threads stay the default

Limits:
- Gears: no undercut or trochoid root fillet (below the base circle the flank runs radially), no helical, internal or bevel gears, no profile shift or backlash.
- Springs: 24-sided wire and 48 segments per turn whatever the tolerance; no closed or ground ends.
- Fasteners: no head chamfer or washer face; modelled threads have no run-out or end chamfer, are right hand coarse only, shade soft at the crests and make about 120 000 triangles for an M8x30 bolt; a nut finds no thread phase by itself (place it a whole number of pitches from the bearing face or turn it).
- Bearings: derived internal geometry (not a catalogue ball set), no cage, seals or ring chamfers; deep groove ball bearings of series 60 and 62 only.
- Components: the gizmo only translates and writes plain millimetres over position expressions; no rotate gizmo; material and part number are set per component.
- Joints: tree only (closed loops such as four-bar linkages need the solver, Phase 6); no joint limits; frames picked from faces are stored as numbers and do not follow later edits of the part; no dragging of joints with the mouse.
- Interference: on demand only, not while a joint is driven or a component dragged; common solids thinner than 0.04 mm (twice the facet tolerance) count as contact; booleans and distances run on the tessellation, so curved faces are measured as facets; only components are checked, not shown bodies; clearance is listed for all pairs up to a limit (no pick of two components).

Remaining:
- F: dependency graph between sketches, features and bodies; stable face references; draft of existing faces about a neutral plane

## Phase 6: Advanced engineering
Mass properties, kinematics, static structural analysis, FEA infrastructure. Every result is labelled
approximate or validated; unvalidated results are never presented as certified.

## Phase 7: Optimization and production
Profiling-driven work: instancing, culling, LOD, mesh caches, binary cache format, parallel tessellation.
STEP, IGES, STL, OBJ, 3MF, glTF, PDF, IFC. Plugin API. Docking, themes, command search. Documentation.
