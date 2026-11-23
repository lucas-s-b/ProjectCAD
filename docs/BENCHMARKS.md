# Benchmark baseline

Single machine, single run of 7 samples per case; a baseline for later comparison, not a performance claim.

Date: 2026-11-16 | profile: release | CPUs: 12 | file size of the 100 000-line project: 33.0 MB

| Case | N | Median ms |
|---|---|---|
| object_snap | 10000 | 0.044 |
| object_snap | 100000 | 1.056 |
| nearest-entity pick (distance_2d) | 100000 | 10.116 |
| intersections, all pairs | 300 | 3.147 |
| solver, line chain | 10 | 0.278 |
| solver, line chain | 50 | 2.359 |
| solver, line chain | 150 | 15.481 |
| solver, line chain | 400 | 178.250 |
| history: 10000 executes + 10000 undos | 10000 | 4.353 |
| save | 100000 | 105.020 |
| load | 100000 | 61.243 |
| to_dxf | 100000 | 98.907 |
| to_svg | 100000 | 72.716 |
| from_dxf | 100000 | 132.174 |
| tessellate spheres | 2000 | 54.042 |
| hatch_lines, 1 m square, spacing 0.5 mm | 1 | 0.123 |
| rebuild plate with 20 cut holes (cold) | 21 features, 21 recomputed | 58.381 |
| the same after changing one hole radius (incremental) | 21 features, 1 recomputed (last hole) | 6.325 |
| rebuild plate with a 10 x 10 hole pattern (cold) | 4 features, 10016 triangles | 523.158 |

Solver rows (the other rows are from the earlier run) re-measured after the switch to sparse normal equations with a sparse Cholesky factorisation (faer), same machine. Before: 0.224 / 7.792 / 175.150 / 3750.094 ms for chains of 10 / 50 / 150 / 400.

## Phase 7 batch A: component assemblies

Release build, same machine, 7 samples per case; "before" is the code at the start of batch A, "after" its end. The
scene rows are debug-build timings from the app itself (`perf.scene_ms` in the MCP `state`, `tests/mcp_phase7a.sh`,
100 components), one sample each.

| Case | N | Before ms | After ms | Speed-up |
|---|---|---|---|---|
| assembly rebuild: a gear and 200 components of it (cold) | 200 x 4568 triangles | 46.252 | 19.043 | 2.43x |
| the same after changing an unused parameter (warm) | 200 components, 0 features recomputed | 43.303 | 0.923 | 46.92x |
| feature edges of every component (each 3D scene build) | 200 | 102.327 | 103.535 | 0.99x |
| mass properties of the assembly | 200 | 5.773 | 5.752 | 1.00x |
| 3D scene build after selecting a component (debug, in-app) | 100 components | 799.7804 | 0.115 | 6954.61x |
| vertices built per scene (count) (debug, in-app) | 100 components | 2019290 | 90 | 22436.56x |

Phase 3 rows re-run after batch A: 59.982 / 6.889 / 545.779 ms (before 59.179 / 6.366 / 543.065; no regression expected: no feature code changed).

## Phase 7 batch B: analyses off the UI thread

Release build, same machine, 7 samples per case; "before" is the code at the start of batch B, "after" its end
(`target/bench_7b_before.txt`, `target/bench_7b_after.txt`). The app rows are debug-build timings from the app
itself: `perf.frame_ms` in the MCP `state`, the longest frame since the previous `state` read
(`tests/mcp_phase7b.sh`), one sample each. Cut-offs: a click that holds the UI thread for 100 ms or more (release)
moves off it; work done every frame stays under 16 ms; a cold open stays under 1 s.

| Case | N | Before ms | After ms | Speed-up |
|---|---|---|---|---|
| static study: steel cantilever 100 x 10 x 10 mm, h 2.45 mm | 29766 unknowns | 413.033 | 416.108 | 0.99x |
| analysis mesh of the same beam (Mesh for analysis) | 6150 elements | 3.745 | 3.744 | 1.00x |
| element loops of that study: stiffness, internal forces, von Mises | 6150 | 16.003 | 15.856 | 1.01x |
| validation benchmarks (Run benchmarks) | 15 cases | 196.341 | 196.536 | 1.00x |
| interference check: 20 gears 80 mm apart | 28 meetings | 318.841 | 320.770 | 0.99x |
| motion study with collision check: a pin turning in a plate hole | 360 steps | 260.335 | 255.638 | 1.02x |
| BVH of every component (first 3D click in an assembly) | 200 | 74.026 | 73.300 | 1.01x |
| bill of materials of the assembly (each frame of the Assembly tab) | 200 | 0.755 | 0.766 | 0.99x |
| cold open: load + rebuild of the 10 x 10 pattern plate | 4 features | 533.665 | 541.342 | 0.99x |
| frame with Run static study (debug, in-app) | beam, 29766 unknowns | 479.621 | 1.436 | 334.04x |
| frame with Run benchmarks (debug, in-app) | 15 cases | 307.917 | 2.335 | 131.89x |
| frame with Mesh for analysis (debug, in-app) | beam, h 2.45 mm | 10.452 | 10.457 | 1.00x |
| frame with Check (debug, in-app) | 20 gears 80 mm apart | 411.580 | 40.395 | 10.19x |
| frame with Run study, collision check (debug, in-app) | pin, 360 steps | 312.947 | 2.800 | 111.76x |
| frame with a 3D click on the assembly (debug, in-app) | 100 components | 1079.897 | 1087.083 | 0.99x |

Gates (from the before rows):
- G1 static study on the worker: "static study ..." >= 100 ms: yes (413.033 ms)
- G2 analysis mesh on the worker: "analysis mesh ..." >= 100 ms: no (3.745 ms)
- G3 motion study on the worker: "motion study with collision check ..." >= 100 ms: yes (260.335 ms)
- G4 interference check on the worker: "interference check ..." >= 100 ms: yes (318.841 ms)
- G5 validation benchmarks on the worker: "validation benchmarks ..." >= 100 ms: yes (196.341 ms)
- G6 picking through one BVH per body: "BVH of every component ..." >= 100 ms: no (74.026 ms)
- G7 parallel FEA element loops: G1 and "element loops ..." >= 0.3 x "static study ...": no (16.003 ms against 0.3 x 413.033 = 123.910 ms)
- BOM: "bill of materials ..." < 16 ms: yes (0.755 ms)
- Cold open: "cold open ..." < 1000 ms: yes (533.665 ms)

Phase 3 and batch A rows re-run after batch B: 60.237 / 6.974 / 540.880 ms (plate, incremental, pattern) and 19.143 / 0.881 / 102.870 / 5.733 ms (assembly cold, warm, feature edges, mass) (`target/bench_7b_after.txt`).

Batch B changed no code under the release rows (the analyses only moved to a thread of their own), so those rows match their before values within noise; the gain is the app rows: the frame of a click on Run static study, Run benchmarks, Check and Run study no longer holds the UI thread. Analysis mesh (G2), picking (G6) and the FEA element loops (G7) failed their gates and stay as they were; the BOM and the cold open are inside their cut-offs.
