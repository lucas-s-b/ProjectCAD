#!/usr/bin/env bash
# End-to-end test of static linear-elastic studies (a bar in tension and under pressure), through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase6e.out
rm -f "$OUT" target/mcp_phase6e_*.png target/mcp_phase6e.cadproj

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

# The faces of the box in the "Study face" list, in the rebuild's face order (read from the widgets of request 16).
FMX="Face 5: plane -X"
FPX="Face 3: plane +X"

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase6e","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. box 20 x 10 x 10 (body 1); no study yet
  cw 3 Rect
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":20,"y":10,"world":true}'
  cw 6 Model
  call 7 click '{"x":10,"y":0,"world":true}'
  st 8 Distance "10 mm"
  cw 9 Extrude
  call 10 state '{}'
  # 2. a steel study made "Custom" with Poisson's ratio 0, h 5 mm
  cw 11 "New static study"
  st 12 "Poisson's ratio" "0"
  st 13 "Study element size" "5 mm"
  call 14 state '{}'
  # 3. fix -X, 1000 N along +X on +X, run
  cw 15 "Study face"
  call 16 widgets '{}'
  cw 17 "$FMX"
  cw 18 "Fix face"
  cw 19 "Study face"
  cw 20 "$FPX"
  st 21 "Force X" "1000"
  st 22 "Force Z" "0"
  cw 23 "Add force"
  cw 24 "Run static study"
  call 25 state '{}'
  call 26 widgets '{}'
  # 4. displacement colours in 3D
  cw 27 Displacement
  cw 28 3D
  call 29 key '{"key":"F"}'
  call 30 screenshot '{}'
  # 5. 10 MPa pressure on +X instead
  cw 31 Model
  cw 32 "Remove load"
  cw 33 "Study face"
  cw 34 "$FPX"
  st 35 Pressure "10"
  cw 36 "Add pressure"
  cw 37 "Run static study"
  call 38 state '{}'
  # 6. no support: refused
  cw 39 "Remove support"
  cw 40 "Run static study"
  call 41 state '{}'
  # 7. undo brings the support back; the study survives save and open
  call 42 key '{"key":"Z","ctrl":true}'
  call 43 save_project '{"path":"target/mcp_phase6e.cadproj"}'
  call 44 new_project '{}'
  call 45 open_project '{"path":"target/mcp_phase6e.cadproj"}'
  call 46 state '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase6e.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# number field $1 of the row on stdin
num() { grep -oE "\"$1\":[-0-9.e]+" | head -1 | cut -d: -f2; }
# $1 >= $2
ge() { awk -v a="$1" -v b="$2" 'BEGIN {exit !(a != "" && a >= b)}'; }
# the fea_study object of state $1 (arrays, no nested braces)
fs() { flat "$1" | grep -oE '"fea_study":\{[^}]*\}'; }
# first number of array field $1 of the row on stdin
first() { grep -oE "\"$1\":\[[-0-9.e]+" | head -1 | cut -d[ -f2; }

[ "$(grep -c . "$OUT")" -ge 45 ] || bad "expected at least 45 responses, got $(grep -c . "$OUT")"
grep "\"id\":30," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase6e_30.png 2>/dev/null
[ "$(head -c 4 target/mcp_phase6e_30.png | tail -c 3)" = "PNG" ] || bad "screenshot 30 is not a PNG"

# 10. no study yet, and a document without studies has no "studies" key
flat 10 | grep -qF '"fea_study":null' || bad "fea_study of state 10 is not null"
flat 10 | grep -qF '"studies"' && bad "state 10 has a studies key"
# 14. the study
for t in '"material":"Custom"' '"poisson":"0"' '"h":"5mm"' '"name":"Study1"'; do
  flat 14 | grep -qF "$t" || bad "no $t in state 14"
done
# 16. the face list
for t in 'plane -X' 'plane +X'; do
  grep "\"id\":16," "$OUT" | grep -qF "$t" || bad "no '$t' in widgets 16"
done
# 25. the tension bar is exact
fs 25 | grep -qF '"accuracy":"approximate(FEA,linearelastic,meshh=5mm)"' || bad "fea_study of state 25: $(fs 25)"
[ "$(fs 25 | num nodes)" = 225 ] || bad "nodes of state 25 is $(fs 25 | num nodes)"
[ "$(fs 25 | num elements)" = 96 ] || bad "elements of state 25 is $(fs 25 | num elements)"
[ "$(fs 25 | num dofs)" = 600 ] || bad "dofs of state 25 is $(fs 25 | num dofs)"
near "$(fs 25 | num max_displacement)" 0.000952381 0.000000001 || bad "max_displacement of state 25 is $(fs 25 | num max_displacement)"
near "$(fs 25 | first max_displacement_at)" 20 0.000001 || bad "max_displacement_at of state 25 is $(fs 25 | first max_displacement_at)"
near "$(fs 25 | num max_von_mises)" 10 0.000001 || bad "max_von_mises of state 25 is $(fs 25 | num max_von_mises)"
near "$(fs 25 | first reaction)" -1000 0.000001 || bad "reaction of state 25 is $(fs 25 | first reaction)"
near "$(fs 25 | first applied)" 1000 0.000001 || bad "applied of state 25 is $(fs 25 | first applied)"
near "$(fs 25 | num residual)" 0 0.000000001 || bad "residual of state 25 is $(fs 25 | num residual)"
fs 25 | grep -qF '"error":null' || bad "error of state 25 is not null"
ge "$(fs 25 | num solve_ms)" 0 || bad "solve_ms of state 25 is $(fs 25 | num solve_ms)"
# 26. the panel
for t in 'Run static study' 'Deformation scale' 'approximate (FEA, linear elastic, mesh h=5 mm)'; do
  grep "\"id\":26," "$OUT" | grep -qF "$t" || bad "no '$t' in widgets 26"
done
# 38. a pressure of 10 MPa gives the same bar
near "$(fs 38 | first reaction)" 1000 0.000001 || bad "reaction of state 38 is $(fs 38 | first reaction)"
near "$(fs 38 | num max_displacement)" 0.000952381 0.000000001 || bad "max_displacement of state 38 is $(fs 38 | num max_displacement)"
# 41. no support
fs 41 | grep -qF '{"error":"fixatleastoneface"}' || bad "fea_study of state 41: $(fs 41)"
# 46. the study survives save and open; the result does not
for t in '"version":29' '"fea_study":null' '"fixed":[{' '{"Pressure":"10"}'; do
  flat 46 | grep -qF "$t" || bad "no $t in state 46"
done

[ "$fail" -eq 0 ] && echo "MCP PHASE6E PASS" || { echo "outputs in $OUT"; exit 1; }
