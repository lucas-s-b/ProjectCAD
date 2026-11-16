#!/usr/bin/env bash
# End-to-end test of the milestone 5 tools (coordinate input, polygon, ellipse, spline, perpendicular snap,
# array, line type) through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase2b.out
SVG=target/p2b.svg
SHOT=target/mcp_phase2b.png
rm -f "$OUT" "$SVG" "$SHOT"

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }

# The scroll first zooms the 2D view out to about 2 px/mm so that world x = 120 is inside the canvas.
{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase2b","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  call 3 scroll '{"x":0,"y":0,"dy":-350,"world":true}'
  # 1. line by typed coordinates
  cw 4 Line
  cw 5 Coordinates
  call 6 type_text '{"text":"0,0"}'
  call 7 key '{"key":"Enter"}'
  call 8 type_text '{"text":"@40<90"}'
  call 9 key '{"key":"Enter"}'
  call 10 state '{}'
  # 2. polygon, 6 sides
  cw 11 Polygon
  call 12 click '{"x":100,"y":0,"world":true}'
  call 13 click '{"x":120,"y":0,"world":true}'
  call 14 state '{}'
  # 3. ellipse
  cw 15 Ellipse
  call 16 click '{"x":0,"y":-60,"world":true}'
  call 17 click '{"x":30,"y":-60,"world":true}'
  call 18 click '{"x":0,"y":-45,"world":true}'
  call 19 state '{}'
  # 4. spline, Enter finishes
  cw 20 Spline
  call 21 click '{"x":-80,"y":0,"world":true}'
  call 22 click '{"x":-60,"y":20,"world":true}'
  call 23 click '{"x":-40,"y":0,"world":true}'
  call 24 key '{"key":"Enter"}'
  call 25 state '{}'
  # 5. perpendicular snap onto a typed line
  cw 26 Line
  cw 27 Coordinates
  call 28 type_text '{"text":"20,0"}'
  call 29 key '{"key":"Enter"}'
  call 30 type_text '{"text":"90,0"}'
  call 31 key '{"key":"Enter"}'
  call 32 click '{"x":50,"y":60,"world":true}'
  call 33 click '{"x":50.4,"y":0.6,"world":true}'
  call 34 state '{}'
  # 6. rectangular array of the ellipse
  cw 35 Select
  call 36 click '{"x":30,"y":-60,"world":true}'
  cw 37 Array
  call 38 set_text '{"name":"dx","text":"80"}'
  cw 42 Rectangular
  call 43 state '{}'
  # 7. dashed layer, export, final screenshot with nothing selected
  cw 44 Type
  cw 45 Dashed
  # clicking empty canvas (a pixel of the 1280x800 screenshot) deselects
  call 46 click '{"x":800,"y":700}'
  call 47 state '{}'
  call 48 export_project "{\"path\":\"$SVG\"}"
  call 49 screenshot '{}'
} | target/debug/cad-app.exe --mcp > "$OUT" 2> target/mcp_phase2b.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# |a - b| below the tolerance (default 1e-6)
near() { awk -v a="$1" -v b="$2" -v t="${3:-1e-6}" 'BEGIN { d = a - b; exit !(d < t && d > -t) }'; }
# numbers of FIELD of the Nth shape object KIND in state ID: shape ID KIND FIELD [N]
shape() {
  flat "$1" | grep -oE "\"$2\":\{[^}]*\}" | sed -n "${4:-1}p" | grep -oE "\"$3\":(\[[^]]*\]|[-0-9.e]+|true|false)" \
    | sed -E 's/^"[a-z_]+"://' | tr -d '[]' | tr ',' ' '
}
# all numbers of the points array of the Nth shape object KIND in state ID
points() {
  flat "$1" | grep -oE "\"$2\":\{[^}]*\}" | sed -n "${3:-1}p" | grep -oE '"points":[][0-9.e+,-]*' \
    | sed 's/"points"://' | tr -d '[]' | tr ',' ' '
}
kinds() { flat "$1" | grep -oE "\"$2\":\{" | wc -l; }

[ "$(grep -c . "$OUT")" -ge 45 ] || bad "expected 45 responses, got $(grep -c . "$OUT")"
for id in 4 5 6 7 8 9 11 13 15 18 20 24 26 29 31 33 35 38 42 45 48; do
  [ -n "$(flat $id)" ] && ! flat $id | grep -q '"isError":true' || bad "call $id failed"
done
# 1. coordinate input: line (0,0)-(0,40)
read -r ax ay bx by <<< "$(shape 10 Line a) $(shape 10 Line b)"
{ near "$ax" 0 && near "$ay" 0 && near "$bx" 0 && near "$by" 40; } || bad "typed line is ($ax,$ay)-($bx,$by)"
# 2. polygon: closed polyline, 6 points, first at (120,0) within the pixel rounding of the click
[ "$(kinds 14 Polyline)" -eq 1 ] || bad "expected 1 Polyline"
[ "$(shape 14 Polyline closed)" = "true" ] || bad "polygon is not closed"
[ "$(points 14 Polyline | wc -w)" -eq 12 ] || bad "polygon does not have 6 points"
read -r px py <<< "$(points 14 Polyline)"
{ near "$px" 120 1e-3 && near "$py" 0 1e-3; } || bad "first polygon vertex is ($px,$py)"
# 3. ellipse: centre (0,-60), rx 30, ry 15, angle 0
[ "$(kinds 19 Ellipse)" -eq 1 ] || bad "expected 1 Ellipse"
read -r cx cy <<< "$(shape 19 Ellipse center)"
{ near "$cx" 0 1e-3 && near "$cy" -60 1e-3; } || bad "ellipse centre ($cx,$cy)"
{ near "$(shape 19 Ellipse rx)" 30 1e-3 && near "$(shape 19 Ellipse ry)" 15 1e-3 && near "$(shape 19 Ellipse angle)" 0; } \
  || bad "ellipse rx/ry/angle $(shape 19 Ellipse rx) $(shape 19 Ellipse ry) $(shape 19 Ellipse angle)"
# 4. spline with 3 points
[ "$(kinds 25 Spline)" -eq 1 ] || bad "expected 1 Spline"
[ "$(points 25 Spline | wc -w)" -eq 6 ] || bad "spline does not have 3 points"
# 5. perpendicular snap: the new line starts at (50,60) and ends at the foot (50,0) on the typed line
read -r ax ay bx by <<< "$(shape 34 Line a 3) $(shape 34 Line b 3)"
{ near "$ax" 50 1e-3 && near "$ay" 60 1e-3 && near "$bx" 50 1e-3 && near "$by" 0 && near "$bx" "$ax"; } \
  || bad "perpendicular line is ($ax,$ay)-($bx,$by)"
read -r ax ay bx by <<< "$(shape 34 Line a 2) $(shape 34 Line b 2)"
{ near "$ax" 20 && near "$ay" 0 && near "$bx" 90 && near "$by" 0; } || bad "typed base line is ($ax,$ay)-($bx,$by)"
# 6. array: a second ellipse at (80,-60)
[ "$(kinds 43 Ellipse)" -eq 2 ] || bad "expected 2 Ellipses after the array, got $(kinds 43 Ellipse)"
read -r cx cy <<< "$(shape 43 Ellipse center 2)"
{ near "$cx" 80 1e-3 && near "$cy" -60 1e-3; } || bad "array copy centre ($cx,$cy)"
# 7. dashed layer and SVG
flat 47 | grep -q '"line_type":"Dashed"' || bad "layer 0 is not Dashed"
[ -f "$SVG" ] && grep -q 'stroke-dasharray' "$SVG" && grep -q '<ellipse' "$SVG" || bad "svg lacks stroke-dasharray or <ellipse"
grep '"id":49,' "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > "$SHOT" 2>/dev/null
[ "$(head -c 4 "$SHOT" | tail -c 3)" = "PNG" ] || bad "screenshot is not a PNG"

[ "$fail" -eq 0 ] && echo "MCP PHASE2B PASS" || { echo "outputs in $OUT"; exit 1; }
