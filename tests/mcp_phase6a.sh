#!/usr/bin/env bash
# End-to-end test of mass properties and accuracy labels, through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase6a.out
rm -f "$OUT" target/mcp_phase6a_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase6a","version":"0"}}}'
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
  call 14 widgets '{}'
  # 2. two steel box components (the second at x 50 copies the first one's material)
  cw 15 Assembly
  cw 16 "Component body"
  cw 17 "Body 1: Extrude 1"
  cw 18 "Add component"
  st 19 Material Steel
  cw 20 "Add component"
  call 21 state '{}'
  # 3. the shaft as a third component, without material
  cw 22 "Component body"
  cw 23 "Body 2: Shaft 2"
  cw 24 "Add component"
  call 25 state '{}'
  call 26 widgets '{}'
  # 4. the centre of mass marker
  cw 27 "Show centre of mass"
  cw 28 3D
  call 29 key '{"key":"F"}'
  call 30 screenshot '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase6a.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# the bodies array of the model: entries {"edges":..,"id":..,"volume":..}
bodies() { flat "$1" | grep -oE '"bodies":\[[^]]*\]'; }
nbodies() { bodies "$1" | grep -oE '"volume"' | wc -l; }
# mass row of body $2 / component $2 / the assembly total
mbody() { flat "$1" | grep -oE "\{\"accuracy\":[^}]*\"body\":$2,[^}]*\}"; }
mcomp() { flat "$1" | grep -oE "\{\"accuracy\":[^}]*\"component\":$2,[^}]*\}"; }
masm() { flat "$1" | grep -oE '"assembly":\{[^}]*\}'; }
# number field $1 of the row on stdin; element $2 of the flat array field $1
num() { grep -oE "\"$1\":[-0-9.e]+" | head -1 | cut -d: -f2; }
elem() { grep -oE "\"$1\":\[[-0-9.e,]*\]" | head -1 | sed "s/\"$1\":\[//; s/\]//" | cut -d, -f"$2"; }

[ "$(grep -c . "$OUT")" -ge 29 ] || bad "expected at least 29 responses, got $(grep -c . "$OUT")"
grep "\"id\":30," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase6a_30.png 2>/dev/null
[ "$(head -c 4 target/mcp_phase6a_30.png | tail -c 3)" = "PNG" ] || bad "screenshot 30 is not a PNG"

# 1. the box (exact, assumed 1 g/cm3) and the shaft (curved)
[ "$(nbodies 13)" -eq 2 ] || bad "expected 2 bodies, got $(nbodies 13)"
mbody 13 1 | grep -qF '"accuracy":"exact","assumed":true' || bad "body 1 row is not exact and assumed"
near "$(mbody 13 1 | num mass)" 8 0.000001 || bad "body 1 mass is $(mbody 13 1 | num mass), expected 8"
near "$(mbody 13 1 | elem com 1)" 20 0.000001 || bad "body 1 com x"
near "$(mbody 13 1 | elem com 3)" 5 0.000001 || bad "body 1 com z"
near "$(mbody 13 1 | elem principal 1)" 333.3333 0.001 || bad "body 1 principal 1"
near "$(mbody 13 1 | elem principal 3)" 1333.3333 0.001 || bad "body 1 principal 3"
mbody 13 2 | grep -qF '"accuracy":"approximate(tessellation)"' || bad "body 2 is not approximate (tessellation)"
near "$(mbody 13 2 | num mass)" 19.37234 0.0001 || bad "body 2 mass is $(mbody 13 2 | num mass), expected 19.37234"
flat 13 | grep -qF '"assembly":null' || bad "assembly is not null without components"
grep "\"id\":14," "$OUT" | grep -qF 'approximate (tessellation)' || bad "no accuracy label in widgets 14"
grep "\"id\":14," "$OUT" | grep -qF 'Show centre of mass' || bad "no 'Show centre of mass' in widgets 14"
# 2. two steel boxes at x 0 and 50
masm 21 | grep -qF '"accuracy":"exact","assumed":false' || bad "assembly 21 is not exact and known"
near "$(masm 21 | num mass)" 125.6 0.0001 || bad "assembly mass is $(masm 21 | num mass), expected 125.6"
near "$(masm 21 | num density)" 7.85 0.000001 || bad "assembly density"
near "$(masm 21 | elem com 1)" 45 0.000001 || bad "assembly com x"
near "$(masm 21 | elem com 2)" 10 0.000001 || bad "assembly com y"
near "$(masm 21 | elem inertia 1)" 5233.3333 0.001 || bad "assembly Ixx"
near "$(masm 21 | elem inertia 2)" 96293.3333 0.001 || bad "assembly Iyy"
near "$(masm 21 | elem inertia 3)" 99433.3333 0.001 || bad "assembly Izz"
near "$(masm 21 | elem inertia 4)" 0 0.001 || bad "assembly Ixy"
near "$(mcomp 21 2 | elem com 1)" 70 0.000001 || bad "component 2 com x"
near "$(mcomp 21 2 | num mass)" 62.8 0.0001 || bad "component 2 mass"
# 3. with the shaft
masm 25 | grep -qF '"accuracy":"approximate(tessellation)","assumed":true' || bad "assembly 25 is not approximate and assumed"
near "$(masm 25 | num mass)" 144.97234 0.0001 || bad "assembly mass with shaft is $(masm 25 | num mass), expected 144.97234"
near "$(masm 25 | elem com 1)" 52.349531 0.0001 || bad "assembly com x with shaft"
near "$(masm 25 | elem com 2)" 8.663722 0.0001 || bad "assembly com y with shaft"
mcomp 25 3 | grep -qF '"assumed":true' || bad "component 3 is not assumed"
# plain labels are not in the widgets list: the accuracy tags and the toggle show the Assembly section is drawn
grep "\"id\":26," "$OUT" | grep -qF 'approximate (tessellation)' || bad "no accuracy label in widgets 26"
grep "\"id\":26," "$OUT" | grep -qF 'Show centre of mass' || bad "no 'Show centre of mass' in widgets 26"

[ "$fail" -eq 0 ] && echo "MCP PHASE6A PASS" || { echo "outputs in $OUT"; exit 1; }
