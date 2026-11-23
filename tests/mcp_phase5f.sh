#!/usr/bin/env bash
# End-to-end test of the dependency graph, drafts and joint frames on faces, through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase5f.out
rm -f "$OUT" target/mcp_phase5f_*.png

# Pixels in the 1280 x 800 window, found with probe runs (click in Select mode, read the status):
# FX,FY: the front face (-Y) of the 40 x 20 x 10 box after the fit (status "Face: plane, area 400"), request 16.
# TX,TY: the top face of the box component, 40 x 20 x 20 mm, after the fit (request 35).
FX=450; FY=490
TX=640; TY=495

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase5f","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. box 40 x 20 x 10 (feature 1 "Extrude 1"), default shaft at x 100 (feature 2 "Shaft 2")
  cw 3 Rect
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":40,"y":20,"world":true}'
  cw 6 Model
  call 7 click '{"x":20,"y":0,"world":true}'
  st 8 Distance "10 mm"
  cw 9 Extrude
  cw 10 Shaft
  cw 11 "Add part"
  st 12 "Part X" "100 mm"
  call 13 state '{}'
  # 2. draft the front face (-Y) 5 deg about sketch 0 (the XY plane)
  st 14 "Draft angle" "5 deg"
  cw 15 "Draft faces"
  call 16 key '{"key":"F"}'
  call 17 state '{}'
  call 18 click "{\"x\":$FX,\"y\":$FY}"
  call 19 key '{"key":"Escape"}'
  call 20 state '{}'
  call 21 widgets '{}'
  call 22 screenshot '{}'
  # 3. box 20 mm tall: the box and its draft are rebuilt, the shaft comes from the cache
  cw 23 "Extrude 1"
  st 24 Distance "20 mm" 1
  call 25 state '{}'
  # 4. components: box (1) and shaft (2), the shaft jointed onto the box's top face
  cw 26 Assembly
  cw 27 "Component body"
  cw 28 "Body 1: Extrude 1"
  cw 29 "Add component"
  cw 30 "Component body"
  cw 31 "Body 2: Shaft 2"
  cw 32 "Add component"
  cw 33 "Add joint"
  st 34 "Child origin X" "100 mm"
  call 35 key '{"key":"F"}'
  call 36 click "{\"x\":$TX,\"y\":$TY}"
  cw 37 "Parent frame from face"
  call 38 state '{}'
  # 5. box 30 mm tall: the shaft follows the top face
  cw 39 Model
  cw 40 "Extrude 1"
  st 41 Distance "30 mm" 1
  call 42 state '{}'
  call 43 screenshot '{}'
} | target/debug/cad-app.exe --mcp > "$OUT" 2> target/mcp_phase5f.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# the bodies array of the model: entries {"edges":..,"id":..,"volume":..}
bodies() { flat "$1" | grep -oE '"bodies":\[[^]]*\]'; }
nbodies() { bodies "$1" | grep -oE '"volume"' | wc -l; }
# volume of body $2 in the bodies array: {"edges":..,"id":$2,"volume":..}
bvol() { bodies "$1" | grep -oE "\"id\":$2,\"volume\":[-0-9.e]+" | cut -d: -f3; }
# the entry of component $2 in the state's components array: {"body":..,"error":..,"id":..,"max":[..],"min":[..],"name":..,"volume":..}
comp() { flat "$1" | grep -oE "\{\"body\":[0-9]+,\"error\":(null|\"[^\"]*\"),\"id\":$2,\"max\":\[[^]]*\],\"min\":\[[^]]*\],\"name\":\"[^\"]*\",\"volume\":[-0-9.e]+\}"; }
ncomps() { flat "$1" | grep -oE '\{"body":[0-9]+,"error":' | wc -l; }
# coordinate $3 (1 x, 2 y, 3 z) of the min corner of component $2
cmin() { comp "$1" "$2" | grep -oE '"min":\[[^]]*\]' | sed 's/"min":\[//; s/\]//' | cut -d, -f"$3"; }

[ "$(grep -c . "$OUT")" -ge 42 ] || bad "expected at least 42 responses, got $(grep -c . "$OUT")"
for n in 22 43; do
  grep "\"id\":$n," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase5f_$n.png 2>/dev/null
  [ "$(head -c 4 target/mcp_phase5f_$n.png | tail -c 3)" = "PNG" ] || bad "screenshot $n is not a PNG"
done

# 1. box and shaft, independent
[ "$(nbodies 13)" -eq 2 ] || bad "expected 2 bodies, got $(nbodies 13)"
near "$(bvol 13 1)" 8000 0.001 || bad "box volume is $(bvol 13 1), expected 8000"
near "$(bvol 13 2)" 19372.34 0.01 || bad "shaft volume is $(bvol 13 2), expected 19372.34"
flat 13 | grep -qF '"depends":{"1":[],"2":[]}' || bad "depends of state 13"
# 2. the draft of the front face
flat 17 | grep -qF '"pick_face":true' || bad "Draft faces did not start picking"
flat 20 | grep -qF '"pick_face":false' || bad "Escape did not end picking"
flat 20 | grep -qF '"feature_states":{"1":"Ok","2":"Ok","3":"Ok"}' || bad "feature states of state 20"
near "$(bvol 20 1)" 7825.0227 0.001 || bad "drafted box volume is $(bvol 20 1), expected 7825.0227"
flat 20 | grep -qF '"depends":{"1":[],"2":[],"3":[1]}' || bad "depends of state 20"
flat 20 | grep -qF '"angle":"5deg","body":1,"faces":[{"at":[' || bad "draft feature is not in the document of state 20"
flat 20 | grep -qE '"key":\{"Plane":\[-?0\.0,-1\.0,-?0\.0\]\}\}\],"sketch":0' || bad "draft face key is not -Y in state 20"
grep "\"id\":21," "$OUT" | grep -qF 'Uses: Extrude 1' || bad "no 'Uses: Extrude 1' in widgets 21"
# 3. box 20 mm: box and draft rebuilt, shaft cached
near "$(bvol 25 1)" 15300.0907 0.001 || bad "box volume at 20 mm is $(bvol 25 1), expected 15300.0907"
near "$(bvol 25 2)" 19372.34 0.01 || bad "shaft volume in state 25 is $(bvol 25 2), expected 19372.34"
flat 25 | grep -qF '"recomputed":2' || bad "recomputed in state 25 is not 2"
# 4. the shaft stands on the box's top face; its min y is the face centroid y (10.874887) minus the faceted radius 9.980267 (the 50-gon has no vertex at -Y)
[ "$(ncomps 38)" -eq 2 ] || bad "expected 2 components, got $(ncomps 38)"
comp 38 2 | grep -qF '"error":null' || bad "component 2 has an error in state 38"
near "$(cmin 38 2 1)" 10 0.001 || bad "shaft min x is $(cmin 38 2 1), expected 10"
near "$(cmin 38 2 2)" 0.894619 0.001 || bad "shaft min y is $(cmin 38 2 2), expected 0.894619"
near "$(cmin 38 2 3)" 20 0.001 || bad "shaft min z is $(cmin 38 2 3), expected 20"
flat 38 | grep -qE '"face":\{"at":\[[^]]*\],"key":\{"Plane":\[-?0\.0,-?0\.0,1\.0\]\}\}' || bad "joint frame has no face reference in state 38"
# 5. box 30 mm: the shaft follows (centroid y 11.312330)
near "$(bvol 42 1)" 22425.2041 0.001 || bad "box volume at 30 mm is $(bvol 42 1), expected 22425.2041"
flat 42 | grep -qF '"recomputed":2' || bad "recomputed in state 42 is not 2"
comp 42 2 | grep -qF '"error":null' || bad "component 2 has an error in state 42"
near "$(cmin 42 2 1)" 10 0.001 || bad "shaft min x is $(cmin 42 2 1), expected 10"
near "$(cmin 42 2 2)" 1.332063 0.001 || bad "shaft min y is $(cmin 42 2 2), expected 1.332063"
near "$(cmin 42 2 3)" 30 0.001 || bad "shaft min z is $(cmin 42 2 3), expected 30"

[ "$fail" -eq 0 ] && echo "MCP PHASE5F PASS" || { echo "outputs in $OUT"; exit 1; }
