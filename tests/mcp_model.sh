#!/usr/bin/env bash
# End-to-end test of the Model tab (parameters, features, solid body) through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_model.out
rm -f "$OUT" target/mcp_model_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"model","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. rectangle and circle
  cw 3 Rect
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":40,"y":20,"world":true}'
  cw 6 Circle
  call 7 click '{"x":20,"y":10,"world":true}'
  call 8 click '{"x":24,"y":10,"world":true}'
  call 9 key '{"key":"Escape"}'
  # 2. Model tab, parameter t = 5 mm
  cw 10 Model
  cw 11 "Param name"
  call 12 type_text '{"text":"t"}'
  cw 13 "Param expression"
  call 14 type_text '{"text":"5 mm"}'
  cw 15 Add
  call 16 state '{}'
  # 3. extrude the rectangle by t
  call 17 click '{"x":20,"y":0,"world":true}'
  cw 18 Distance
  call 19 key '{"key":"A","ctrl":true}'
  call 20 type_text '{"text":"t"}'
  cw 21 Extrude
  call 22 state '{}'
  # 4. cut the circle
  call 23 click '{"x":24,"y":10,"world":true}'
  cw 24 Operation
  cw 25 Cut
  cw 26 Extrude
  call 27 state '{}'
  # 5. t = 10 mm
  cw 28 "Param t"
  call 29 key '{"key":"A","ctrl":true}'
  call 30 type_text '{"text":"10 mm"}'
  call 31 key '{"key":"Enter"}'
  call 32 state '{}'
  # 6. suppress the second feature, undo
  cw 33 S 1
  call 34 state '{}'
  call 35 key '{"key":"Z","ctrl":true}'
  call 36 state '{}'
  # 7. 3D view, fit
  cw 37 3D
  call 38 key '{"key":"F"}'
  call 39 screenshot '{}'
  # 8. unknown name in the second feature's distance, undo
  cw 40 "Extrude 2"
  cw 41 Distance 1
  call 42 key '{"key":"A","ctrl":true}'
  call 43 type_text '{"text":"zz"}'
  call 44 key '{"key":"Enter"}'
  call 45 state '{}'
  call 46 key '{"key":"Z","ctrl":true}'
  call 47 state '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_model.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
volume() { flat "$1" | grep -oE '"volume":[-0-9.e+]+' | cut -d: -f2; }
states() { flat "$1" | grep -oE '"feature_states":\{[^}]*\}'; }
# $1 and $2 equal within relative tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t * (b < 0 ? -b : b))}'; }

[ "$(grep -c . "$OUT")" -ge 40 ] || bad "expected 40 responses, got $(grep -c . "$OUT")"
grep '"id":39,' "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_model_3d.png 2>/dev/null
[ "$(head -c 4 target/mcp_model_3d.png | tail -c 3)" = "PNG" ] || bad "3D screenshot is not a PNG"

# 2. parameter
flat 16 | grep -q '"side_tab":"Model"' || bad "side tab is not Model"
flat 16 | grep -qE '"param_values":\{"t":5(\.0)?\}' || bad "param t is not 5"
# 3. extrude
states 22 | grep -q '{"1":"Ok"}' || bad "expected one Ok feature ($(states 22))"
near "$(volume 22)" 4000 1e-6 || bad "volume after the extrude is $(volume 22), expected 4000"
# 4. cut
v4=$(volume 27)
near "$v4" "$(awk 'BEGIN {print 4000 - 3.141592653589793 * 16 * 5}')" 0.003 || bad "volume after the cut is $v4"
states 27 | grep -q '{"1":"Ok","2":"Ok"}' || bad "expected two Ok features ($(states 27))"
# 5. t = 10 mm
near "$(volume 32)" "$(awk -v v="$v4" 'BEGIN {print 2 * v}')" 1e-6 || bad "volume after t = 10 mm is $(volume 32), expected twice $v4"
# 6. suppress and undo
near "$(volume 34)" 8000 1e-6 || bad "volume with the cut suppressed is $(volume 34)"
states 34 | grep -q '"2":"Suppressed"' || bad "second feature not Suppressed ($(states 34))"
states 36 | grep -q '{"1":"Ok","2":"Ok"}' || bad "states after undo are $(states 36)"
near "$(volume 36)" "$(volume 32)" 1e-9 || bad "volume after undo is $(volume 36), expected $(volume 32)"
# 8. unknown name
states 45 | grep -q '"2":"Error' || bad "second feature has no Error state ($(states 45))"
near "$(volume 45)" 8000 1e-6 || bad "volume with the broken cut is $(volume 45), expected 8000"
states 47 | grep -q '{"1":"Ok","2":"Ok"}' || bad "states after the last undo are $(states 47)"
near "$(volume 47)" "$(volume 32)" 1e-9 || bad "volume after the last undo is $(volume 47)"

[ "$fail" -eq 0 ] && echo "MCP MODEL PASS" || { echo "outputs in $OUT"; exit 1; }
