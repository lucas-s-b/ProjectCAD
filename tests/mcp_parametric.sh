#!/usr/bin/env bash
# End-to-end test of parameter-driven sketch constraints and the solid through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_parametric.out
rm -f "$OUT"

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
# fill the text field with this name and occurrence index, then Enter
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }
# select the line through this world point; a second point is added with shift
pick() { call "$1" click "{\"x\":$2,\"y\":$3,\"world\":true}"; }
add() { call "$1" click "{\"x\":$2,\"y\":$3,\"world\":true,\"shift\":true}"; }

# Midpoints of the four lines: bottom (19,0.5), right (38.5,10), top (20,20), left (0.5,10.5); they move by under a millimetre while the constraints are added.
{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"parametric","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. parameters w = 40 mm, h = 20 mm
  cw 3 Model
  cw 4 "Param name"
  call 5 type_text '{"text":"w"}'
  cw 6 "Param expression"
  call 7 type_text '{"text":"40 mm"}'
  cw 8 Add
  cw 9 "Param name"
  call 10 type_text '{"text":"h"}'
  cw 11 "Param expression"
  call 12 type_text '{"text":"20 mm"}'
  cw 13 Add
  call 14 state '{}'
  # 2. a rough rectangle drawn with the Line tool
  cw 15 Sketch
  cw 16 Line
  call 17 click '{"x":0,"y":0,"world":true}'
  call 18 click '{"x":38,"y":1,"world":true}'
  call 19 click '{"x":38,"y":1,"world":true}'
  call 20 click '{"x":39,"y":19,"world":true}'
  call 21 click '{"x":39,"y":19,"world":true}'
  call 22 click '{"x":1,"y":21,"world":true}'
  call 23 click '{"x":1,"y":21,"world":true}'
  call 24 click '{"x":0,"y":0,"world":true}'
  call 25 key '{"key":"Escape"}'
  call 26 state '{}'
  # 3. Coincident on the four corners
  pick 27 19 0.5
  add 28 38.5 10
  cw 29 Coincident
  pick 30 38.5 10
  add 31 20 20
  cw 32 Coincident
  pick 33 20 20
  add 34 0.5 10.5
  cw 35 Coincident
  pick 36 0.5 10.5
  add 37 19 0.5
  cw 38 Coincident
  # 4. Horizontal on the bottom and top lines, Vertical on the right and left lines
  pick 39 19 0.5
  cw 40 Horizontal
  pick 41 20 20
  cw 42 Horizontal
  pick 43 38.5 10
  cw 44 Vertical
  pick 45 0.5 10.5
  cw 46 Vertical
  call 47 state '{}'
  # 5. Length of the bottom line = w, of the left line = h
  pick 48 19 0.5
  cw 49 Length
  st 50 "Constraint value" w 0
  pick 51 0.5 10.5
  cw 52 Length
  st 53 "Constraint value" h 1
  call 54 state '{}'
  # 6. extrude the four lines by 5 mm
  cw 55 Model
  pick 56 19 0.5
  add 57 38.5 10
  add 58 20 20
  add 59 0.5 10.5
  cw 60 Distance
  call 61 key '{"key":"A","ctrl":true}'
  call 62 type_text '{"text":"5 mm"}'
  cw 63 Extrude
  call 64 state '{}'
  # 7. w = 60 mm
  cw 65 "Param w"
  call 66 key '{"key":"A","ctrl":true}'
  call 67 type_text '{"text":"60 mm"}'
  call 68 key '{"key":"Enter"}'
  call 69 state '{}'
  # 8. undo
  call 70 key '{"key":"Z","ctrl":true}'
  call 71 state '{}'
} | target/debug/cad-app.exe --mcp > "$OUT" 2> target/mcp_parametric.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
volume() { flat "$1" | grep -oE '"volume":[-0-9.e+]+' | cut -d: -f2; }
# "ax ay bx by" of line entity $2 in the state response $1
line_pts() {
  flat "$1" | grep -oE "\"id\":$2,\"layer\":[0-9]+,(\"meta\":\{[^}]*\},)?\"name\":\"[^\"]*\",\"shape\":\{\"Line\":\{\"a\":\[[^]]*\],\"b\":\[[^]]*\]" \
    | grep -oE '\[[^]]*\]' | tr -d '[]' | tr ',' ' ' | tr '\n' ' '
}
len() { line_pts "$1" "$2" | awk '{print sqrt(($3 - $1) ^ 2 + ($4 - $2) ^ 2)}'; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
n_lines() { flat "$1" | grep -o '"Line":{' | wc -l; }
n_constraints() { flat "$1" | sed -E 's/.*"constraints":(.*),"entities":\{.*/\1/' | grep -oE '"[0-9]+":\{"[A-Za-z]+"' | wc -l; }

[ "$(grep -c . "$OUT")" -ge 70 ] || bad "expected 70 responses, got $(grep -c . "$OUT")"
flat 14 | grep -qE '"param_values":\{"h":20(\.0)?,"w":40(\.0)?\}' || bad "parameters w and h are missing or wrong"
[ "$(n_lines 26)" -eq 4 ] || bad "expected 4 lines after drawing, got $(n_lines 26)"
[ "$(n_constraints 47)" -eq 8 ] || bad "expected 8 constraints (4 Coincident, 2 Horizontal, 2 Vertical), got $(n_constraints 47)"
[ "$(n_constraints 54)" -eq 10 ] || bad "expected 10 constraints after the two lengths, got $(n_constraints 54)"
flat 54 | grep -q '"constraint_exprs":{[^}]*"w"' || bad "no constraint expression w after setting the value"
flat 54 | grep -q '"constraint_exprs":{[^}]*"h"' || bad "no constraint expression h after setting the value"
near "$(len 54 1)" 40 1e-6 || bad "bottom line length is $(len 54 1), expected 40"
near "$(len 54 4)" 20 1e-6 || bad "left line length is $(len 54 4), expected 20"
near "$(volume 64)" 4000 1e-6 || bad "volume is $(volume 64), expected 4000"
near "$(len 69 1)" 60 1e-6 || bad "bottom line length after w = 60 mm is $(len 69 1), expected 60"
near "$(volume 69)" 6000 1e-6 || bad "volume after w = 60 mm is $(volume 69), expected 6000"
near "$(len 71 1)" 40 1e-6 || bad "bottom line length after undo is $(len 71 1), expected 40"
near "$(volume 71)" 4000 1e-6 || bad "volume after undo is $(volume 71), expected 4000"

[ "$fail" -eq 0 ] && echo "MCP PARAMETRIC PASS" || { echo "outputs in $OUT"; exit 1; }
