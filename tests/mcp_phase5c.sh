#!/usr/bin/env bash
# End-to-end test of joints, grounded components, exploded views and export of placed components, through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase5c.out
SAVED=target/mcp_phase5c.cadproj
OBJ=target/mcp_phase5c.obj
STEP=target/mcp_phase5c.step
rm -f "$OUT" "$SAVED" "$OBJ" "$STEP" target/mcp_phase5c_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase5c","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. gear 1 and bolt 2 (default values; the bolt at x 60)
  cw 3 Model
  cw 4 Gear
  cw 5 "Add part"
  cw 6 Bolt
  cw 7 "Add part"
  st 8 "Part X" "60 mm"
  call 9 state '{}'
  # 2. components gear #1, gear #2, bolt #1; gear #1 grounded
  cw 10 Assembly
  cw 11 "Component body"
  cw 12 "Body 1: Gear 1"
  cw 13 "Add component"
  cw 14 "Add component"
  cw 15 "Component body"
  cw 16 "Body 2: Bolt 2"
  cw 17 "Add component"
  cw 18 "Gear 1 #1"
  cw 19 Grounded
  call 20 state '{}'
  # 3. joint 1: gear #2 on gear #1 (fixed, coincident), then revolute about Z at x 40 (centre distance of two m2 z20 gears)
  cw 21 "Gear 1 #2"
  cw 22 "Joint parent"
  cw 23 "Parent: Gear 1 #1"
  cw 24 "Add joint"
  call 25 state '{}'
  cw 26 "Joint type"
  cw 27 "Type: Revolute"
  st 28 "Parent origin X" "40 mm"
  st 29 "Joint value" "0 deg"
  call 30 state '{}'
  # 4. joint 2: the bolt fixed on gear #2, 10 mm off its axis, head bearing face on its top face (z 10); the bolt axis is at x 60 of its body
  cw 31 "Bolt 2 #1"
  cw 32 "Joint parent"
  cw 33 "Parent: Gear 1 #2"
  cw 34 "Add joint"
  st 35 "Parent origin X" "10 mm"
  st 36 "Parent origin Z" "10 mm"
  st 37 "Child origin X" "60 mm"
  call 38 state '{}'
  # 5. driving joint 1 to 90 deg swings gear #2 and the bolt with it
  cw 39 "Joint 1"
  st 40 "Joint value" "90 deg"
  call 41 state '{}'
  # 6. refusals: a grounded child; a frame from a face before any face was clicked
  cw 42 "Gear 1 #1"
  cw 43 "Joint parent"
  cw 44 "Parent: Bolt 2 #1"
  cw 45 "Add joint"
  call 46 state '{}'
  cw 47 "Parent frame from face"
  call 48 state '{}'
  # 7. exploded view: the bolt 50 mm up, at factor 1 and 0.5, with trail lines
  cw 49 3D
  call 50 key '{"key":"F"}'
  cw 51 "Bolt 2 #1"
  cw 52 "Add explode step"
  cw 53 "Exploded view"
  st 54 "Explode factor" 1
  call 55 state '{}'
  st 56 "Explode factor" 0.5
  cw 57 "Trail lines"
  call 58 state '{}'
  call 59 screenshot '{}'
  # 8. OBJ and STEP of the placed components (assembled, not exploded)
  call 60 export_project "{\"path\":\"$OBJ\"}"
  call 61 export_project "{\"path\":\"$STEP\"}"
  # 9. deleting gear #2 takes both joints with it; undo restores everything
  cw 62 "Gear 1 #2"
  cw 63 "Delete component"
  call 64 state '{}'
  call 65 key '{"key":"Z","ctrl":true}'
  call 66 state '{}'
  # 10. save, new project, open
  call 67 save_project "{\"path\":\"$SAVED\"}"
  call 68 new_project '{}'
  call 69 open_project "{\"path\":\"$SAVED\"}"
  call 70 state '{}'
  call 71 screenshot '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase5c.err

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
# the entry of component $2 in the state's components array: {"body":..,"error":..,"id":..,"max":[..],"min":[..],"name":..,"volume":..}
comp() { flat "$1" | grep -oE "\{\"body\":[0-9]+,\"error\":(null|\"[^\"]*\"),\"id\":$2,\"max\":\[[^]]*\],\"min\":\[[^]]*\],\"name\":\"[^\"]*\",\"volume\":[-0-9.e]+\}"; }
ncomps() { flat "$1" | grep -oE '\{"body":[0-9]+,"error":' | wc -l; }
# coordinate $3 (1 x, 2 y, 3 z) of the min corner of component $2
cmin() { comp "$1" "$2" | grep -oE '"min":\[[^]]*\]' | sed 's/"min":\[//; s/\]//' | cut -d, -f"$3"; }
# number of entries in the state's joints array: {"child":..,"error":..,"id":..,"kind":..,"parent":..,"value":..}
njoints() { flat "$1" | grep -oE '\{"child":[0-9]+,"error":' | wc -l; }

[ "$(grep -c . "$OUT")" -ge 70 ] || bad "expected at least 70 responses, got $(grep -c . "$OUT")"
for n in 59 71; do
  grep "\"id\":$n," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > "target/mcp_phase5c_$n.png" 2>/dev/null
  [ "$(head -c 4 "target/mcp_phase5c_$n.png" | tail -c 3)" = "PNG" ] || bad "screenshot $n is not a PNG"
done

# 1. two bodies
[ "$(nbodies 9)" -eq 2 ] || bad "expected 2 bodies, got $(nbodies 9)"
# 2. three components, gear #1 grounded
[ "$(ncomps 20)" -eq 3 ] || bad "expected 3 components, got $(ncomps 20)"
flat 20 | grep -qF '"grounded":[1]' || bad "gear #1 is not grounded"
near "$(cmin 20 2 1)" 32 0.05 || bad "component 2 min x is $(cmin 20 2 1), expected 32"
near "$(cmin 20 3 1)" 53.5 0.001 || bad "bolt component min x is $(cmin 20 3 1), expected 53.5"
# 3. joint 1
[ "$(njoints 25)" -eq 1 ] || bad "expected 1 joint, got $(njoints 25)"
flat 25 | grep -qF '{"child":2,"error":null,"id":1,"kind":"Fixed","parent":1,"value":"0"}' || bad "joint 1 of state 25"
near "$(cmin 25 2 1)" -22 0.05 || bad "gear #2 min x on gear #1 is $(cmin 25 2 1), expected -22"
flat 30 | grep -qF '"id":1,"kind":"Revolute","parent":1,"value":"0deg"}' || bad "joint 1 of state 30"
near "$(cmin 30 2 1)" 18 0.05 || bad "gear #2 min x at 40 is $(cmin 30 2 1), expected 18"
near "$(cmin 30 2 3)" 0 0.001 || bad "gear #2 min z is $(cmin 30 2 3), expected 0"
# 4. joint 2
[ "$(njoints 38)" -eq 2 ] || bad "expected 2 joints, got $(njoints 38)"
flat 38 | grep -qF '{"child":3,"error":null,"id":2,"kind":"Fixed","parent":2,"value":"0"}' || bad "joint 2 of state 38"
near "$(cmin 38 3 1)" 43.5 0.001 || bad "bolt min x on gear #2 is $(cmin 38 3 1), expected 43.5"
near "$(cmin 38 3 2)" -7.505553 0.001 || bad "bolt min y on gear #2 is $(cmin 38 3 2), expected -7.505553"
near "$(cmin 38 3 3)" -20 0.001 || bad "bolt min z on gear #2 is $(cmin 38 3 3), expected -20"
# 5. driving joint 1
flat 41 | grep -qF '"value":"90deg"' || bad "joint 1 value in state 41"
near "$(cmin 41 2 1)" 18 0.05 || bad "gear #2 min x at 90 deg is $(cmin 41 2 1), expected 18"
near "$(cmin 41 3 1)" 32.494447 0.001 || bad "bolt min x at 90 deg is $(cmin 41 3 1), expected 32.494447"
near "$(cmin 41 3 2)" 3.5 0.001 || bad "bolt min y at 90 deg is $(cmin 41 3 2), expected 3.5"
near "$(cmin 41 3 3)" -20 0.001 || bad "bolt min z at 90 deg is $(cmin 41 3 3), expected -20"
# 6. refusals
[ "$(njoints 46)" -eq 2 ] || bad "expected 2 joints after the refused one, got $(njoints 46)"
flat 46 | grep -qF 'invalidjoint:thechildisgrounded' || bad "no grounded-child status in state 46"
flat 48 | grep -qF 'clickafaceofGear1#1inthe3Dviewfirst' || bad "no click-a-face status in state 48"
flat 48 | grep -qF '"component_face":null' || bad "component_face is not null in state 48"
# 7. exploded view
flat 55 | grep -qF '"explode":{"factor":1.0,"offsets":{"3":[0.0,0.0,50.0]}}' || bad "explode of state 55"
near "$(cmin 55 3 3)" -20 0.001 || bad "state shows an exploded placement: bolt min z $(cmin 55 3 3)"
flat 58 | grep -qF '"offsets":{"3":[0.0,0.0,25.0]}' || bad "explode offsets of state 58"
# 8. export
[ -f "$OBJ" ] || bad "no OBJ file"
[ "$(grep -c '^o ' "$OBJ")" -eq 3 ] || bad "OBJ has $(grep -c '^o ' "$OBJ") groups, expected 3"
grep -qxF 'o Gear 1 #1' "$OBJ" || bad "OBJ group Gear 1 #1"
grep -qxF 'o Gear 1 #2' "$OBJ" || bad "OBJ group Gear 1 #2"
grep -qxF 'o Bolt 2 #1' "$OBJ" || bad "OBJ group Bolt 2 #1"
! grep -q '^o Body_' "$OBJ" || bad "OBJ has a Body_ group"
near "$(awk '$1 == "v" && (m == "" || $4 < m) { m = $4 } END { print m }' "$OBJ")" -20 0.001 || bad "OBJ min z is not -20 (the bolt is not placed)"
[ -f "$STEP" ] || bad "no STEP file"
grep -qF "'Bolt 2 #1'" "$STEP" || bad "STEP solid Bolt 2 #1"
grep -qF "'Gear 1 #2'" "$STEP" || bad "STEP solid Gear 1 #2"
# 9. delete and undo
[ "$(ncomps 64)" -eq 2 ] || bad "expected 2 components after delete, got $(ncomps 64)"
flat 64 | grep -qF '"joints":[]' || bad "joints remain after delete"
near "$(cmin 64 3 1)" 53.5 0.001 || bad "bolt did not return to its own position: min x $(cmin 64 3 1)"
[ "$(ncomps 66)" -eq 3 ] || bad "expected 3 components after undo, got $(ncomps 66)"
[ "$(njoints 66)" -eq 2 ] || bad "expected 2 joints after undo, got $(njoints 66)"
near "$(cmin 66 3 1)" 32.494447 0.001 || bad "bolt min x after undo is $(cmin 66 3 1), expected 32.494447"
# 10. the saved file comes back the same
[ "$(ncomps 70)" -eq 3 ] || bad "expected 3 components after open, got $(ncomps 70)"
[ "$(njoints 70)" -eq 2 ] || bad "expected 2 joints after open, got $(njoints 70)"
flat 70 | grep -qF '"grounded":[1]' || bad "grounded flag lost after open"
near "$(cmin 70 3 1)" 32.494447 0.001 || bad "bolt min x after open is $(cmin 70 3 1), expected 32.494447"
near "$(cmin 70 2 1)" 18 0.05 || bad "gear #2 min x after open is $(cmin 70 2 1), expected 18"

[ "$fail" -eq 0 ] && echo "MCP PHASE5C PASS" || { echo "outputs in $OUT"; exit 1; }
