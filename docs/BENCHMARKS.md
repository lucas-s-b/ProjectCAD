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
