#!/usr/bin/env bash
# End-to-end test of components and the bill of materials, through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase5b.out
SAVED=target/mcp_phase5b.cadproj
CSV=target/mcp_phase5b.csv
rm -f "$OUT" "$SAVED" "$CSV" target/mcp_phase5b_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase5b","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. gear 1 and bolt 2 (default values; the bolt at x 60)
  cw 3 Model
  cw 4 Gear
  cw 5 "Add part"
  cw 6 Bolt
  cw 7 "Add part"
  st 8 "Part X" "60 mm"
  call 9 state '{}'
  # 2. three components of the gear, one of the bolt
  cw 10 Assembly
  cw 11 "Component body"
  cw 12 "Body 1: Gear 1"
  cw 13 "Add component"
  cw 14 "Add component"
  cw 15 "Add component"
  cw 16 "Component body"
  cw 17 "Body 2: Bolt 2"
  cw 18 "Add component"
  call 19 state '{}'
  # 3. component 2 at y 60, turned one tooth pitch (18 deg); components 1 and 2 of steel
  cw 20 "Gear 1 #2"
  st 21 "Component Y" "60 mm"
  st 22 "Rotation Z" "18 deg"
  st 23 Material Steel
  cw 24 "Gear 1 #1"
  st 25 Material Steel
  call 26 state '{}'
  # 4. the bill of materials as CSV
  call 27 export_project "{\"path\":\"$CSV\"}"
  # 5. the gizmo moves the bolt component along X
  cw 28 3D
  call 29 key '{"key":"F"}'
  cw 30 "Bolt 2 #1"
  call 31 state '{}'
  # gizmo of state 31 is [[541,457],[723,516],[629,377],[537,275]] (origin, X, Y, Z tips): drag from the middle of the X arrow one arrow length along it
  call 32 drag '{"x1":632,"y1":486,"x2":814,"y2":545,"button":"primary"}'
  call 33 state '{}'
  # 6. delete component 3, undo brings it back
  cw 34 "Gear 1 #3"
  cw 35 "Delete component"
  call 36 state '{}'
  call 37 key '{"key":"Z","ctrl":true}'
  call 38 state '{}'
  # 7. save, new project, open: the components come back
  call 39 save_project "{\"path\":\"$SAVED\"}"
  call 40 new_project '{}'
  call 41 open_project "{\"path\":\"$SAVED\"}"
  call 42 state '{}'
  call 43 screenshot '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase5b.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# value of key $1 in the entry on stdin
val() { grep -oE "\"$1\":[-0-9.e+]+" | cut -d: -f2; }
# the bodies array of the model: entries {"edges":..,"id":..,"volume":..}
bodies() { flat "$1" | grep -oE '"bodies":\[[^]]*\]'; }
nbodies() { bodies "$1" | grep -oE '"volume"' | wc -l; }
body() { bodies "$1" | grep -oE "\{\"edges\":[0-9]+,\"id\":$2,[^}]*\}"; }
# the entry of component $2 in the state's components array: {"body":..,"error":..,"id":..,"max":[..],"min":[..],"name":..,"volume":..}
comp() { flat "$1" | grep -oE "\{\"body\":[0-9]+,\"error\":(null|\"[^\"]*\"),\"id\":$2,\"max\":\[[^]]*\],\"min\":\[[^]]*\],\"name\":\"[^\"]*\",\"volume\":[-0-9.e]+\}"; }
ncomps() { flat "$1" | grep -oE '\{"body":[0-9]+,"error":' | wc -l; }
# coordinate $3 (1 x, 2 y, 3 z) of the min corner of component $2
cmin() { comp "$1" "$2" | grep -oE '"min":\[[^]]*\]' | sed 's/"min":\[//; s/\]//' | cut -d, -f"$3"; }

[ "$(grep -c . "$OUT")" -ge 42 ] || bad "expected at least 42 responses, got $(grep -c . "$OUT")"
grep "\"id\":43," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase5b_43.png 2>/dev/null
[ "$(head -c 4 target/mcp_phase5b_43.png | tail -c 3)" = "PNG" ] || bad "screenshot 43 is not a PNG"

# 1. two bodies
[ "$(nbodies 9)" -eq 2 ] || bad "expected 2 bodies, got $(nbodies 9)"
# 2. three gear components and one bolt component side by side
[ "$(ncomps 19)" -eq 4 ] || bad "expected 4 components, got $(ncomps 19)"
near "$(cmin 19 1 1)" -22 0.05 || bad "component 1 min x is $(cmin 19 1 1), expected -22"
near "$(cmin 19 2 1)" 32 0.05 || bad "component 2 min x is $(cmin 19 2 1), expected 32"
near "$(cmin 19 3 1)" 86 0.05 || bad "component 3 min x is $(cmin 19 3 1), expected 86"
near "$(cmin 19 4 1)" 53.5 0.001 || bad "bolt component min x is $(cmin 19 4 1), expected 53.5"
near "$(comp 19 1 | val volume)" "$(body 9 1 | val volume)" 0.01 || bad "component 1 volume differs from its body"
flat 19 | grep -qF '"designation":"Spurgearm2z20b10","item":1,"mass":null,"material":"","name":"Gear1","part_number":"","quantity":3' || bad "bom row 1 of state 19"
flat 19 | grep -qF '"designation":"ISO4017M8x30","item":2,' || bad "bom row 2 of state 19"
# 3. component 2 moved and turned, two gears of steel
near "$(cmin 26 2 2)" 38 0.05 || bad "component 2 min y is $(cmin 26 2 2), expected 38"
near "$(cmin 26 2 1)" 32 0.05 || bad "component 2 min x is $(cmin 26 2 1), expected 32"
near "$(flat 26 | grep -oE '"item":1,"mass":[0-9.]+' | cut -d: -f3)" 92.3 0.3 || bad "steel gear mass is not about 92.3 g"
flat 26 | grep -qF '"material":"Steel","name":"Gear1","part_number":"","quantity":2' || bad "bom row 1 of state 26"
flat 26 | grep -qF '"item":2,"mass":null,"material":"","name":"Gear1","part_number":"","quantity":1' || bad "bom row 2 of state 26"
flat 26 | grep -qF '"designation":"ISO4017M8x30","item":3,' || bad "bom row 3 of state 26"
# 4. CSV
[ -f "$CSV" ] || bad "no CSV file"
grep -qF 'Item,Part,Designation,Material,Part number,Quantity,Mass (g)' "$CSV" || bad "CSV header"
grep -qE '^1,Gear 1,Spur gear m2 z20 b10,Steel,,2,92\.[0-9]$' "$CSV" || bad "CSV row 1"
grep -qxF '2,Gear 1,Spur gear m2 z20 b10,,,1,' "$CSV" || bad "CSV row 2"
grep -qxF '3,Bolt 2,ISO 4017 M8x30,,,1,' "$CSV" || bad "CSV row 3"
[ "$(wc -l < "$CSV")" -eq 4 ] || bad "CSV has $(wc -l < "$CSV") lines, expected 4"
# 5. the gizmo
flat 31 | grep -q '"component_sel":4' || bad "component 4 is not selected"
flat 31 | grep -q '"gizmo":\[\[' || bad "no gizmo for the component"
awk -v x="$(cmin 33 4 1)" 'BEGIN { exit !(x > 54.5) }' || bad "bolt component did not move along X: min x $(cmin 33 4 1)"
near "$(cmin 33 4 2)" -7.505553 0.001 || bad "bolt component moved in y: min y $(cmin 33 4 2)"
# 6. delete and undo
[ "$(ncomps 36)" -eq 3 ] || bad "expected 3 components after delete, got $(ncomps 36)"
flat 36 | grep -qF '"designation":"ISO4017M8x30","item":2,' || bad "bom row 2 of state 36"
[ "$(ncomps 38)" -eq 4 ] || bad "expected 4 components after undo, got $(ncomps 38)"
# 7. the saved file comes back the same
[ "$(ncomps 42)" -eq 4 ] || bad "expected 4 components after open, got $(ncomps 42)"
near "$(cmin 42 2 2)" 38 0.05 || bad "component 2 min y after open is $(cmin 42 2 2), expected 38"
flat 42 | grep -qF '"designation":"ISO4017M8x30","item":3,' || bad "bom row 3 of state 42"

[ "$fail" -eq 0 ] && echo "MCP PHASE5B PASS" || { echo "outputs in $OUT"; exit 1; }
