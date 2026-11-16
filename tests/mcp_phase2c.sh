#!/usr/bin/env bash
# End-to-end test of the milestone 6 tools (radius, angle and leader tools, hatch from Properties, DXF export and
# import) through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase2c.out
DXF=target/p2c.dxf
SHOT=target/mcp_phase2c.png
rm -f "$OUT" "$DXF" "$SHOT"

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }

# The scroll first zooms the 2D view out to about 2 px/mm so that world x = -80 .. 90 is inside the canvas.
{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase2c","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  call 2 scroll '{"x":0,"y":0,"dy":-350,"world":true}'
  # 1. circle (0,0) r 20, then a radius dimension attached at 45 degrees
  cw 3 Circle
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":20,"y":0,"world":true}'
  cw 6 "Radius dim"
  call 7 click '{"x":0,"y":20,"world":true}'
  call 8 click '{"x":30,"y":30,"world":true}'
  call 9 state '{}'
  # 2. hatch of the selected circle
  cw 10 Select
  call 11 click '{"x":20,"y":0,"world":true}'
  cw 12 Hatch
  call 13 state '{}'
  # 3. angular dimension
  cw 14 "Angle dim"
  call 15 click '{"x":60,"y":0,"world":true}'
  call 16 click '{"x":90,"y":0,"world":true}'
  call 17 click '{"x":60,"y":30,"world":true}'
  call 18 state '{}'
  # 4. leader, Enter finishes
  cw 19 Leader
  call 20 click '{"x":-40,"y":0,"world":true}'
  call 21 click '{"x":-60,"y":20,"world":true}'
  call 22 click '{"x":-80,"y":20,"world":true}'
  call 23 key '{"key":"Enter"}'
  call 24 state '{}'
  # 5. screenshot with nothing selected, export, new project, import
  cw 25 Select
  # clicking empty canvas (a pixel of the 1280x800 screenshot) deselects
  call 26 click '{"x":800,"y":700}'
  call 27 screenshot '{}'
  call 28 export_project "{\"path\":\"$DXF\"}"
  call 29 new_project '{}'
  call 30 import_project "{\"path\":\"$DXF\"}"
  call 31 state '{}'
} | target/debug/cad-app.exe --mcp > "$OUT" 2> target/mcp_phase2c.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
near() { awk -v a="$1" -v b="$2" -v t="${3:-1e-6}" 'BEGIN { d = a - b; exit !(d < t && d > -t) }'; }
# numbers of FIELD of the Nth shape object KIND in state ID: shape ID KIND FIELD [N]
shape() {
  flat "$1" | grep -oE "\"$2\":\{[^}]*\}" | sed -n "${4:-1}p" | grep -oE "\"$3\":(\[[^]]*\]|[-0-9.e]+|\"[^\"]*\"|true|false)" \
    | sed -E 's/^"[a-z_]+"://' | tr -d '[]' | tr ',' ' '
}
# all numbers of the array FIELD of the first shape object KIND in state ID
nums() {
  flat "$1" | grep -oE "\"$2\":\{[^}]*\}" | sed -n 1p | grep -oE "\"$3\":[][0-9.e+,-]*" | sed -E 's/^"[a-z_]+"://' | tr -d '[]' | tr ',' ' '
}
count() { flat "$1" | grep -oE "\"$2\":\{" | wc -l; }

[ "$(grep -c . "$OUT")" -ge 30 ] || bad "expected 30 responses, got $(grep -c . "$OUT")"
for id in 3 4 5 6 7 8 10 11 12 14 15 16 17 19 20 21 22 23 25 26 28 29 30; do
  [ -n "$(flat $id)" ] && ! flat $id | grep -q '"isError":true' || bad "call $id failed"
done
# 1. radial dimension: centre (0,0), radius 20, angle 45 degrees
[ "$(count 9 RadialDim)" -eq 1 ] || bad "expected 1 RadialDim"
flat 9 | grep -q '"name":"Radialdimension' || bad "entity is not named Radial dimension"
read -r cx cy <<< "$(shape 9 RadialDim center)"
{ near "$cx" 0 1e-3 && near "$cy" 0 1e-3; } || bad "radial centre ($cx,$cy)"
near "$(shape 9 RadialDim radius)" 20 1e-3 || bad "radial radius $(shape 9 RadialDim radius)"
near "$(shape 9 RadialDim angle)" 0.7853981634 1e-3 || bad "radial angle $(shape 9 RadialDim angle)"
# 2. hatch: 64 boundary points (OUTLINE_SEGMENTS of the circle flattening), spacing 2
[ "$(count 13 Hatch)" -eq 1 ] || bad "expected 1 Hatch"
[ "$(nums 13 Hatch boundary | wc -w)" -eq 128 ] || bad "hatch boundary has $(( $(nums 13 Hatch boundary | wc -w) / 2 )) points, expected 64"
near "$(shape 13 Hatch spacing)" 2 || bad "hatch spacing $(shape 13 Hatch spacing)"
# 3. angular dimension: sweep 90 degrees
[ "$(count 18 AngularDim)" -eq 1 ] || bad "expected 1 AngularDim"
flat 18 | grep -q '"name":"Angulardimension' || bad "entity is not named Angular dimension"
near "$(shape 18 AngularDim sweep)" 1.5707963268 1e-3 || bad "angular sweep $(shape 18 AngularDim sweep)"
# 4. leader: 3 points, default text
[ "$(count 24 Leader)" -eq 1 ] || bad "expected 1 Leader"
[ "$(nums 24 Leader points | wc -w)" -eq 6 ] || bad "leader does not have 3 points"
[ "$(shape 24 Leader text)" = '"Text"' ] || bad "leader text is $(shape 24 Leader text)"
# 5. the screenshot is a PNG; export, then import into a new project
grep '"id":27,' "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > "$SHOT" 2>/dev/null
[ "$(head -c 4 "$SHOT" | tail -c 3)" = "PNG" ] || bad "screenshot is not a PNG"
[ -s "$DXF" ] || bad "dxf was not written"
flat 30 | grep -qE 'Imported[0-9]+entities' || flat 30 | grep -q '"status":"Imported' || bad "import reply has no 'Imported N entities' status: $(flat 30 | cut -c1-300)"
[ "$(count 31 Circle)" -ge 1 ] || bad "no Circle after the import"
near "$(shape 31 Circle radius)" 20 1e-3 || bad "imported circle radius $(shape 31 Circle radius)"

[ "$fail" -eq 0 ] && echo "MCP PHASE2C PASS" || { echo "outputs in $OUT"; exit 1; }
