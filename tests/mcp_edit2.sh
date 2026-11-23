#!/usr/bin/env bash
# End-to-end test of split, join, align, distribute, pinned dragging, constraint glyphs and construction geometry
# through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_edit2.out
SVG=target/mcp_edit2.svg
SHOT=target/mcp_edit2.png
rm -f "$OUT" "$SVG" "$SHOT"

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"edit2","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. split a line, then join the pieces again
  cw 3 Line
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":40,"y":0,"world":true}'
  cw 6 Split
  call 7 click '{"x":10,"y":0,"world":true}'
  call 8 state '{}'
  cw 9 Select
  call 10 click '{"x":5,"y":0,"world":true}'
  call 11 click '{"x":25,"y":0,"world":true,"shift":true}'
  cw 12 Join
  call 13 state '{}'
  # 2. three circles: align the centres on one y, then distribute them in x
  call 14 new_project '{}'
  cw 15 Circle
  call 16 click '{"x":0,"y":0,"world":true}'
  call 17 click '{"x":2,"y":0,"world":true}'
  call 18 click '{"x":10,"y":5,"world":true}'
  call 19 click '{"x":12,"y":5,"world":true}'
  call 20 click '{"x":30,"y":2,"world":true}'
  call 21 click '{"x":32,"y":2,"world":true}'
  cw 22 Select
  call 23 click '{"x":2,"y":0,"world":true}'
  call 24 click '{"x":12,"y":5,"world":true,"shift":true}'
  call 25 click '{"x":32,"y":2,"world":true,"shift":true}'
  cw 26 "Align centre Y"
  call 27 state '{}'
  cw 28 "Distribute X"
  call 29 state '{}'
  # 3. two joined lines, A horizontal: dragging B drags A along
  call 30 new_project '{}'
  cw 31 Line
  call 32 click '{"x":0,"y":0,"world":true}'
  call 33 click '{"x":20,"y":0,"world":true}'
  call 34 click '{"x":20,"y":0,"world":true}'
  call 35 click '{"x":30,"y":10,"world":true}'
  cw 36 Select
  call 37 click '{"x":10,"y":0,"world":true}'
  call 38 click '{"x":25,"y":5,"world":true,"shift":true}'
  cw 39 Coincident
  call 40 click '{"x":10,"y":0,"world":true}'
  cw 41 Horizontal
  call 42 drag '{"x1":30,"y1":10,"x2":30,"y2":20,"world":true}'
  call 43 state '{}'
  call 44 click '{"x":800,"y":700}'
  call 45 screenshot '{}'
  # 4. construction geometry stays out of the export
  call 46 new_project '{}'
  cw 47 Line
  call 48 click '{"x":0,"y":0,"world":true}'
  call 49 click '{"x":40,"y":0,"world":true}'
  cw 50 Circle
  call 51 click '{"x":0,"y":20,"world":true}'
  call 52 click '{"x":5,"y":20,"world":true}'
  cw 53 Select
  call 54 click '{"x":20,"y":0,"world":true}'
  cw 55 Construction
  call 56 state '{}'
  call 57 export_project "{\"path\":\"$SVG\"}"
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_edit2.err

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
same() { awk -v s="$1" -v w="$2" -v t="${3:-1e-6}" 'BEGIN { n = split(s, a, " "); m = split(w, b, " "); if (n != m) exit 1; for (i = 1; i <= n; i++) { d = a[i] - b[i]; if (d > t || d < -t) exit 1 } }'; }

[ "$(grep -c . "$OUT")" -ge 55 ] || bad "expected 55 responses, got $(grep -c . "$OUT")"
for id in 3 6 7 9 12 14 15 22 26 28 30 31 36 39 41 42 46 47 50 53 55 57; do
  [ -n "$(flat $id)" ] && ! flat $id | grep -q '"isError":true' || bad "call $id failed"
done
# 1. split gives (0,0)-(10,0) and (10,0)-(40,0); join gives one polyline through (0,0), (10,0), (40,0)
[ "$(kinds 8 Line)" -eq 2 ] || bad "expected 2 lines after the split, got $(kinds 8 Line)"
same "$(shape 8 Line a 1) $(shape 8 Line b 1)" "0 0 10 0" || bad "first split piece is $(shape 8 Line a 1) $(shape 8 Line b 1)"
same "$(shape 8 Line a 2) $(shape 8 Line b 2)" "10 0 40 0" || bad "second split piece is $(shape 8 Line a 2) $(shape 8 Line b 2)"
[ "$(kinds 13 Polyline)" -eq 1 ] && [ "$(kinds 13 Line)" -eq 0 ] || bad "expected one Polyline and no Line after Join"
[ "$(points 13 Polyline | wc -w)" -eq 6 ] || bad "joined polyline does not have 3 points"
{ same "$(points 13 Polyline)" "0 0 10 0 40 0" || same "$(points 13 Polyline)" "40 0 10 0 0 0"; } || bad "joined polyline is $(points 13 Polyline)"
# 2. align centre Y: one y; distribute X: x = 0, 15, 30
for n in 1 2 3; do
  read -r cx cy <<< "$(shape 27 Circle center $n)"
  near "$cy" 2.5 1e-3 || bad "circle $n centre y is $cy after Align centre Y"
  read -r cx cy <<< "$(shape 29 Circle center $n)"
  near "$cx" "$((n * 15 - 15))" 1e-3 || bad "circle $n centre x is $cx after Distribute X"
done
# 3. B moved up by 10, A follows, joint kept, A horizontal
same "$(shape 43 Line a 2) $(shape 43 Line b 2)" "20 10 30 20" 1e-3 || bad "dragged line B is $(shape 43 Line a 2) $(shape 43 Line b 2)"
read -r ax ay bx by <<< "$(shape 43 Line a 1) $(shape 43 Line b 1)"
read -r sx sy <<< "$(shape 43 Line a 2)"
{ near "$bx" "$sx" && near "$by" "$sy"; } || bad "joint of A end ($bx,$by) and B start ($sx,$sy) broke"
near "$ay" "$by" || bad "A is not horizontal after the drag ($ay vs $by)"
flat 43 | grep -q '"Coincident"' && flat 43 | grep -q '"Horizontal"' || bad "constraints are missing after the drag"
grep '"id":45,' "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > "$SHOT" 2>/dev/null
[ "$(head -c 4 "$SHOT" | tail -c 3)" = "PNG" ] || bad "screenshot is not a PNG"
# 4. construction line: flagged in the state, absent from the SVG, circle still there
[ "$(flat 56 | grep -oE '"construction":true' | wc -l)" -eq 1 ] || bad "expected exactly one construction entity"
[ -f "$SVG" ] && grep -q '<circle' "$SVG" && ! grep -q '<line' "$SVG" || bad "svg must hold the circle and no <line"

[ "$fail" -eq 0 ] && echo "MCP EDIT2 PASS" || { echo "outputs in $OUT"; exit 1; }
