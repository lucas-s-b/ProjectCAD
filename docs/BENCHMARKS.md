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
