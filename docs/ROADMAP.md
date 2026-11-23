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
- The feature history is linear: changing an early feature recomputes everything after it (since Phase 5 batch F only the features that read a body it changed).
- Ellipse and spline cannot be part of a profile. Crossing profile loops are not detected.
- STEP: a body with any face other than a plane or cylinder, or an edge other than a line, circle or ellipse, is exported faceted.
- Shell: the offset tool is a faceted sphere, so walls next to curved or slanted faces are slightly thicker than asked; an open shell needs a thickness of at most about a quarter of the body depth along the opening direction.
- Fillets and chamfers: no blending at corners where filleted edges meet; a stored edge is found again as the nearest edge between faces pointing the same ways as when picked; if the picked edge is gone, another such edge may be blended. Edges from files before version 13 are picked points and report "edge not found" when moved. A rim fillet on a round hole carries a
  few percent volume error at the 0.02 mm body tolerance (mesh slivers between facets and the true circle).
- Tapered extrude: walls of arcs and circles are faceted (no analytic cone), so STEP writes them faceted; a draft that turns an edge around or makes the moved outlines cross is refused; collinear overlaps of the moved outlines are not detected.
- Smooth loft: uniform Catmull-Rom through the section points, 8 rings between two sections, no self-intersection check; with two sections it is the ruled loft.

Deferred to later phases:
- Dependency graph between sketches, features and bodies instead of the linear history: done in Phase 5 batch F; references for shell faces and for faces that tilt, split or merge: Phase 7 (topological naming on the exact kernel)
- Draft of existing faces about a neutral plane: done in Phase 5 batch F for planar faces whose neighbours are planes at right angles to them; drafts next to curved faces, corner blending where filleted edges meet, fillets on other edge types: Phase 7 (surfaces must be re-intersected or blended with free-form patches)
- Loft sections with holes; self-intersection checks for sweep and loft: Phase 7
- Trimmed faces and analytic curves beyond plane and cylinder (cones, spheres, tori, cylinder-cylinder); booleans on exact geometry instead of the mesh: Phase 7
- NURBS curves and surfaces: Phase 7
- STEP export of cones, spheres, tori and free-form faces (planes and cylinders done): Phase 7 (cones and spheres done in Phase 7 batch C). Phase 5 parts are mesh-boolean solids checked with parry on meshes, so they do not need it first.

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
- IFC export: storeys, walls, openings, spaces, slabs, roofs, stairs, property sets from entity metadata (the design above: classes from `cad_arch::ifc_class`, quantities from the solids, GlobalIds derived at export): Phase 7 (export formats) (done in Phase 7 batch C)
- Roofs on concave outlines (straight skeleton), dormers, roof openings, walls trimmed to the roof: Phase 7
- Stairs with landings, turns or spirals, railings, the stair opening in the slab above: Phase 7
- Plan cut height, walls attached to the level above, openings in slabs, dragging openings along their wall: Phase 7

## Phase 5: Mechanical CAD (done; deferred items listed below)
Parametric generators (gears, shafts, bearings, springs, fasteners, threads), assemblies, joints,
collision and interference checks (parry), exploded views, bill of materials.

Done (unit-tested; part generators, components, the bill of materials, joints, exploded views, the interference check, bearings, modelled threads, the dependency graph, drafts and joint frames on faces checked in the running app through MCP):
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
- [x] Dependency graph between sketches, features and bodies: a feature's cache key holds its own evaluated inputs and the versions of the bodies it reads, not the features before it, so editing a feature recomputes only the features that read a body it changed; the feature editor shows what a feature uses ("Uses: Extrude 1"); `depends` and `recomputed` in the MCP `state`
- [x] Stable face references (`FaceRef`: which way a face points and a point on it), found again on every rebuild; used by drafts and joint frames
- [x] Draft of existing planar faces about a neutral plane (the plane of a sketch; pull direction its normal): "Draft faces" picks faces of one body in the 3D view, the angle is an expression, the faces follow when earlier features move them; refused next to curved faces; file format version 23
- [x] Joint frames taken from a face follow that face when the part changes ("Parent frame from face", "Child frame from face"); typed numbers replace the face

Limits:
- Gears: no undercut or trochoid root fillet (below the base circle the flank runs radially), no helical, internal or bevel gears, no profile shift or backlash.
- Springs: 24-sided wire and 48 segments per turn whatever the tolerance; no closed or ground ends.
- Fasteners: no head chamfer or washer face; modelled threads have no run-out or end chamfer, are right hand coarse only, shade soft at the crests and make about 120 000 triangles for an M8x30 bolt; a nut finds no thread phase by itself (place it a whole number of pitches from the bearing face or turn it).
- Bearings: derived internal geometry (not a catalogue ball set), no cage, seals or ring chamfers; deep groove ball bearings of series 60 and 62 only.
- Components: the gizmo only translates and writes plain millimetres over position expressions; no rotate gizmo; material and part number are set per component.
- Joints: tree only in Phase 5 (closed loops, joint limits and slider dragging added in Phase 6); a frame picked from a face follows it, a typed frame stays where it is put; no dragging of joints with the mouse.
- Interference: on demand only, not while a joint is driven or a component dragged; common solids thinner than 0.04 mm (twice the facet tolerance) count as contact; booleans and distances run on the tessellation, so curved faces are measured as facets; only components are checked, not shown bodies; clearance is listed for all pairs up to a limit (no pick of two components).

- Dependency graph: features still rebuild one after another in list order; fillets, chamfers, shells and mirrors of all bodies read every body before them; features on one body form a chain, so editing the first recomputes the rest.
- Draft: planar faces parallel to the pull direction only, whose neighbouring faces are planes at right angles to them (prism walls, pockets, bosses); no draft next to fillets or cylinders, no split-line draft; only flipped triangles are checked, not other self-intersections; a sketch or edge picked on a drafted face does not follow a later angle change.
- Face references: a planar face is found under its picked point along its normal, a cylinder by the nearest centroid; faces that tilt, split or merge are not tracked.

Deferred to later phases:
- (done in Phase 6, except dragging a jointed component with the 3D gizmo: Phase 7) Closed kinematic loops, joint limits, dragging joints, interference while a joint moves
- Components that reference other .cadproj files (`source: File(path)`), a rotate gizmo for components: Phase 7
- References for shell faces and for faces that tilt, split or merge; drafts next to curved faces; corner blending where filleted edges meet: Phase 7 (exact kernel)
- Gears with undercut, profile shift, backlash, helical, internal or bevel teeth; springs with closed or ground ends; fastener head chamfers and washer faces; thread run-out, end chamfers, left-hand and fine threads, tapped holes; bearing cages, seals, angular contact, roller and thrust bearings: Phase 7
- Booleans, interference and clearance on exact geometry instead of facets: Phase 7

## Phase 6: Advanced engineering (done; deferred items listed below)
Mass properties, kinematics, static structural analysis, FEA infrastructure. Every result is labelled
approximate or validated; unvalidated results are never presented as certified.

Done (unit-tested; mass properties, motion studies, closed loops, the analysis mesh, static studies, the safety factor and the validation report checked in the running app through MCP):
- [x] Accuracy labels on analysis results: "exact" (the computation is exact for the geometry), "approximate (<reason>)", "validated against <case>"; shown beside every result and in the MCP `state`; nothing is presented as certified
- [x] Mass properties of bodies and placed components: volume, mass, centre of mass, inertia tensor about the centre of mass, principal moments and axes, from the closed triangle mesh by the divergence theorem (exact for bodies with planar faces, approximate (tessellation) for curved ones); components use their BOM material, bodies and unknown materials an assumed 1 g/cm³ (flagged); assembly total about the assembly centre of mass with the parallel axis theorem; Model and Assembly tab "Mass properties" sections, centre of mass marker in 3D, `mass` in the MCP `state`
- [x] Joint limits: optional min/max expressions on revolute and slider joints; a value outside fails the child's placement ("joint 1: 120 deg is above the maximum 90 deg"), never clamped; file format 24 (older files load unchanged)
- [x] Motion studies: drive one revolute or slider joint from a start to an end value in up to 360 steps (exact joint walk per step), trace a point of a component, play back in 3D (step slider, play/pause; display only); optional collision check per step on the pairs where one side moves, reporting the first colliding step and pair with the interference region in red and the time taken; CSV export (`export_motion` in MCP); `motion` in the MCP `state`; positions "exact", collisions "approximate (sampled at the steps)"
- [x] Closed-loop joints: a joint marked "Closes loop" is not walked; Levenberg-Marquardt (the sketch solver's loop, shared as `cad_constraints::least_squares`) sets the values of joints marked "Free joint" so that its frames meet; errors for under-constrained loops, over-constrained loops and loops that do not close; limits refuse solved values; motion studies drive linkages, starting each step from the last solution; a Grashof crank-rocker four-bar matches the analytic position equation through 360° (listed in the validation report as "validated against four-bar position analysis"); solved values in the joint list and the MCP `state` (`solved`), labelled "approximate (loop closed to 1e-9 mm)"; file format 25 (older files load unchanged)
- [x] Dragging joints: a "Drag" slider and value in the joint editor shows the pose in 3D while dragging (loops solved per frame) and writes the value, with the free joints' solved values, on release
- [x] Tetrahedral mesher for analysis (our own; no pure-Rust mesher exists): a grid of cells at most the element size over the body's box, each cut into 6 Kuhn tetrahedra (conforming), kept when the centroid is inside (ray parity), boundary nodes snapped onto the surface unless that would invert an element or drop its radius ratio below 0.15, quadratic 10-node tetrahedra (C3D10 node order, straight sides); quality (smallest radius ratio and dihedral angle, inverted count 0 by construction) and the mesh volume against the body's, labelled "approximate (mesh)"; node sets of faces found from face references (for E's loads and supports); Model tab "Analysis mesh" section (body, element size, "Mesh for analysis"), the mesh in 3D as boundary faces with edges or shrunken elements, `fea_mesh` in the MCP `state`
- [x] Static linear-elastic FEA: studies stored in the document (file format 26; body, element size, material with E and ν from the material table or custom, fixed faces and force or pressure loads on face references, optional gravity); quadratic tetrahedra with 4-point Gauss integration, consistent face and body loads, sparse assembly with the fixed DOFs eliminated, faer sparse Cholesky (at most 30 000 unknowns); nodal displacements and averaged von Mises stresses, maxima with their locations, support reactions and the out-of-balance; Model tab "Static study" section (faces picked from a list that follows the 3D face selection), the deformed body coloured by von Mises or displacement with a legend, `fea_study` in the MCP `state`; every result labelled "approximate (FEA, linear elastic, mesh h=…)"
- [x] Yield strength in the material table and in studies (file format 27; empty for older studies), safety factor = yield strength / largest nodal von Mises stress in the study results and `fea_study`, under the study's approximate label
- [x] Validation: a benchmark suite (`cad_document::benchmarks`) builds analytic cases with the app's own commands and runs the app's code on them: a steel cantilever at three element sizes (tip deflection against Timoshenko within 3 %: −2.4 %, −1.1 %, −0.8 %; bending stress at mid-span within 3 %: +2.5 %, +0.9 %, +0.2 %), a bar in tension (exact to round-off), mass properties of a box (exact) and a cylinder (facets, within 0.5 % and 1 %), the four-bar position analysis (within 1e-6°); `cargo run --release -p cad-bench -- validate` writes docs/VALIDATION.md (15 of 15 within tolerance), `cargo test` runs the same cases; the Model tab's "Validation report" runs them on demand and is the only place results read "validated against <case>" (studies you run stay approximate); `validation` in the MCP `state`

Limits:
- Mass properties: bodies have no material (1 g/cm³ assumed); curved faces are integrated on their facets (a 64-gon cylinder is 0.16 % light); no inertia about a chosen point or axis in the UI; principal axes are listed, not drawn.
- Motion: a collision between two steps is not seen; curved faces are checked on their facets; the study runs on a worker thread since Phase 7 batch B (times shown); picking, the centre of mass marker and the Interference section stay on the assembled pose; one joint per study; studies are not saved.
- Loops: one solve for all loops (an error fails every component that depends on a free joint); the solver can land on the other assembly branch when start values are far off; limits are checked after the solve, not used by it; closing joints have no value or limits; the drag slider has a fixed range (±180 deg, ±100 mm); dragging a jointed component with the 3D gizmo is not available (Phase 7).
- Mesh: a regular grid, so sharp edges that do not lie on grid lines are rounded off (a box turned 30° loses 4 % of its volume at 2.5 mm; a cylinder 10 mm in radius 0.9 %); boundary nodes whose move would spoil an element stay off the surface (counted); straight-sided elements; meshing runs on the UI thread with at most 50 000 grid cells; one body at a time, not components; the mesh is not saved; face node sets have no picker yet (E).
- Static studies: linear elastic, small displacements, isotropic material; supports hold every displacement of a face (no rollers or prescribed displacements); loads are uniform over a face; straight-sided elements on the mesher's grid (sharp edges off the grid are rounded off); at most 30 000 unknowns, run on a worker thread; the safety factor compares the yield strength with the largest nodal von Mises stress only (no fatigue, buckling or notch judgement; peaks at sharp re-entrant edges grow with refinement); supports and loads are listed, not drawn.
- Validation: 15 fixed cases on a box, a cylinder and the four-bar; a user study is never labelled validated, however close to a case; against Timoshenko the fully clamped 3D cantilever keeps about −0.8 % at h = 2.5 mm; no Lamé cylinder, gravity or stress-concentration case yet.

Deferred to later phases:
- (done in Phase 7 batch B) Running static studies, motion studies, the interference check and the validation benchmarks on a worker thread
- Mass properties, interference and collisions during motion on exact geometry instead of facets: Phase 7 (exact kernel)
- Dragging a jointed component with the 3D gizmo along its free motion: Phase 7
- Modal, thermal, buckling and fatigue analysis; contact; nonlinear materials and large displacements: later
- Curved quadratic elements, adaptive mesh refinement, meshing components or several bodies in one study, saving meshes: later
- Roller supports, prescribed non-zero displacements, non-uniform loads; load and support glyphs in 3D; exporting study results: later
- More benchmarks (Lamé thick-walled cylinder, which needs roller supports and curved elements; gravity; pressure on curved faces; stress concentrations); recognising a user's study as a benchmark case: later
- Continuous collision detection between motion samples, motion studies saved in the document, loops solved per independent group, joint limits as solver constraints: later

## Phase 7: Optimization and production (in progress)
Profiling-driven work: instancing, culling, LOD, mesh caches, binary cache format, parallel tessellation.
STEP, IGES, STL, OBJ, 3MF, glTF, PDF, IFC. Plugin API. Docking, themes, command search. Documentation.

Done (batch A; numbers in docs/BENCHMARKS.md; checked in the running app through MCP):
- [x] Measured first: every optimisation starts with a `cad-bench` case (or the in-app scene timer, `perf` in the MCP `state`) and ends with before and after numbers; `cargo run -p cad-bench -- assembly <path> [n]` writes the benchmark assembly
- [x] Components are placed in parallel (rayon) and reused while their body and placement are unchanged; `RebuildResult::placements`
- [x] Instanced 3D drawing: each body's triangles and edges go to the GPU once; shown bodies and components are instances, so a selection, gizmo, explode, motion or drag frame no longer rebuilds triangles

Done (batch B; numbers in docs/BENCHMARKS.md; checked in the running app through MCP):
- [x] Measured first: `cad-bench` analysis, pick, BOM and cold-open cases, the in-app longest-frame timer (`perf.frame_ms`), gates at 100 ms per click, 16 ms per frame, 1 s per cold open
- [x] Static studies, motion studies, interference checks and the validation benchmarks run on a worker thread: one job at a time, a progress line with "Cancel job", any edit drops the job; MCP replies wait for it (`jobs` in the state). The analysis mesh (3.7 ms), the one-BVH-per-body picking (74 ms for 200 components) and rayon in the FEA element loops (16 ms of a 413 ms study) failed their gates and stay as they were

Done (batch C; unit-tested, every exporter reads its output back; checked in the running app through MCP):
- [x] glTF 2.0 binary export (.glb): each solid once, components as nodes with their placements, metres, Y up, normals, one grey material
- [x] 3MF export: one object per solid, one build item per body or component with its transform, millimetres (own stored zip writer with CRC-32)
- [x] IFC4 export of the architectural model: project, site, building, a storey per level; walls, doors, windows (with the openings that void their walls), floors and rooms as extrusions, roofs and stairs as faceted B-Reps; property sets from `Pset_X.Prop` metadata, net volumes, room areas and perimeters, GlobalIds derived from the ids
- [x] STL and OBJ import as a mesh body in the feature list (closed meshes only; welded, inside-out meshes turned, sharp at creases over 30 degrees); file format version 28
- [x] STEP export of cones and spheres as exact surfaces (shafts with chamfers, revolved parts)

Limits of batch C:
- glTF and 3MF write the shown bodies and components only (no walls, no colours per body, no textures); glTF repeats a vertex per triangle corner.
- IFC: levels must lie on XY; doors and windows are boxes (no frame or leaf); mechanical bodies and components are not exported; GlobalIds change when the document is renamed.
- Imported meshes are stored in the project file as JSON arrays (about 1.5 times the binary STL size); open meshes and units other than millimetres are refused or not converted.
- STEP: tori, whole spheres and free-form faces stay faceted.

Remaining:
- D: docking, themes, command search, rotate gizmo for components, dragging jointed components with the gizmo
- E: plugin API (out-of-process plugins over the MCP protocol, `cad-plugin-api`)
- F: user guide, README, release v0.2.0 (the tag push is left to the user)

Won't do in this project:
- A NURBS kernel, exact booleans and everything that needs them: topological naming of faces that tilt, split or merge; corner blending and fillets on other edges; drafts next to curved faces; loft holes and sweep/loft self-intersection checks; mass, interference and collisions on exact geometry (facet errors are measured and labelled)
- STEP and IGES import, STEP of tori and free-form faces, IGES export (need NURBS; STEP is read everywhere IGES was), 3D PDF
- LOD and hidden-line rendering, unless batch B measures GPU-bound frames after culling
- Mass properties, BOM, frustum culling, a binary rebuild cache, rayon in patterns, mirrors, the mesher, tessellation, feature edges and the FEA element loops, a worker for the analysis mesh and a BVH per source body for picking: measured or read unnecessary in batch B (docs/BENCHMARKS.md)
- The feature-catalogue items deferred in Phases 2, 4, 5 and 6 (2D blocks and drafting extras, roof, stair and opening extras, gear, spring, thread and bearing variants, further analysis types): modelling features, not optimisation or production
- Components referencing other project files; a 2D spatial index (measured unnecessary)
