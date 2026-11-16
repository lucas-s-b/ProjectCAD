#!/usr/bin/env bash
# End-to-end test of sketches on planes and several bodies through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_sketch.out
rm -f "$OUT" target/mcp_sketch_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }

# The pick in the 3D view is a pixel of the 1280x800 screenshot, taken after F with the plate seen from the front:
# (600,400) is a point on the top face. Everything else is clicked by widget name.
{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"sketch","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. rectangle in sketch 0, extruded by 5 mm
  cw 3 Rect
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":40,"y":20,"world":true}'
  call 6 key '{"key":"Escape"}'
  cw 7 Model
  call 8 click '{"x":20,"y":0,"world":true}'
  cw 9 Distance
  call 10 key '{"key":"A","ctrl":true}'
  call 11 type_text '{"text":"5 mm"}'
  cw 12 Extrude
  call 13 state '{}'
  # 2. sketch on the top face of the plate
  cw 14 "Sketch on face"
  call 15 state '{}'
  call 16 key '{"key":"F"}'
  call 17 screenshot '{}'
  call 18 click '{"x":600,"y":400}'
  call 19 state '{}'
  # 3. circle in sketch 1
  cw 20 Circle
  call 21 click '{"x":20,"y":10,"world":true}'
  call 22 click '{"x":24,"y":10,"world":true}'
  call 23 key '{"key":"Escape"}'
  call 24 state '{}'
  # 4. blind hole: cut the circle 2 mm downwards (flip)
  call 25 click '{"x":24,"y":10,"world":true}'
  cw 26 Distance
  call 27 key '{"key":"A","ctrl":true}'
  call 28 type_text '{"text":"2 mm"}'
  cw 29 Operation
  cw 30 Cut
  cw 31 Flip
  cw 32 Extrude
  call 33 state '{}'
  # 5. sketch 2 on the XZ plane, a rectangle as a new body
  cw 34 "Base plane"
  cw 35 XZ
  cw 36 "Add sketch"
  call 37 state '{}'
  cw 38 Rect
  call 39 click '{"x":50,"y":0,"world":true}'
  call 40 click '{"x":60,"y":10,"world":true}'
  call 41 key '{"key":"Escape"}'
  call 42 click '{"x":55,"y":0,"world":true}'
  cw 43 Distance
  call 44 key '{"key":"A","ctrl":true}'
  call 45 type_text '{"text":"5 mm"}'
  cw 46 Operation
  cw 47 "New body"
  cw 48 Flip
  cw 49 Extrude
  call 50 state '{}'
  # 6. 3D view, fit
  cw 51 3D
  call 52 key '{"key":"F"}'
  call 53 screenshot '{}'
  # orbit half a turn (right-button drag, 314 px) to see the block and the sketch rectangle from the +Y side
  call 58 drag '{"x1":500,"y1":600,"x2":814,"y2":600,"button":"secondary"}'
  call 59 screenshot '{}'
  # 7. back to sketch 0 in the 2D view
  cw 54 "Sketch 0"
  cw 55 2D
  call 56 state '{}'
  call 57 screenshot '{}'
} | target/debug/cad-app.exe --mcp > "$OUT" 2> target/mcp_sketch.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# total volume of all bodies
volume() { flat "$1" | grep -oE '"body":\{[^}]*\}' | grep -oE '"volume":[-0-9.e+]+' | cut -d: -f2; }
nbodies() { flat "$1" | grep -oE '"bodies":\[[^]]*\]' | grep -o '"id":' | wc -l; }
field() { flat "$1" | grep -oE "\"$2\":(\"[^\"]*\"|[a-z0-9]+)" | tail -1 | cut -d: -f2; }
# $1 and $2 equal within relative tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t * (b < 0 ? -b : b))}'; }
shot() { grep "\"id\":$1," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > "$2" 2>/dev/null; [ "$(head -c 4 "$2" | tail -c 3)" = "PNG" ] || bad "screenshot $1 is not a PNG"; }

[ "$(grep -c . "$OUT")" -ge 50 ] || bad "expected 50 responses, got $(grep -c . "$OUT")"
shot 17 target/mcp_sketch_pick.png
shot 53 target/mcp_sketch_3d.png
shot 59 target/mcp_sketch_3d_back.png
shot 57 target/mcp_sketch_2d.png

# 1. plate
near "$(volume 13)" 4000 1e-6 || bad "plate volume is $(volume 13), expected 4000"
# 2. sketch on face
[ "$(field 15 pick_face)" = true ] && [ "$(field 15 view)" = '"ThreeD"' ] || bad "Sketch on face did not start picking in the 3D view"
[ "$(field 19 sketch)" = 1 ] || bad "active sketch after the pick is $(field 19 sketch), expected 1"
[ "$(field 19 view)" = '"TwoD"' ] && [ "$(field 19 pick_face)" = false ] || bad "pick did not return to the 2D view"
plane=$(flat 19 | grep -oE '"1":\{"id":1,"name":"Sketch1","plane":\{"Fixed":\{[^}]*\}\}' | grep -oE '\[[^]]*\]' | tr -d '[]' | tr ',' ' ' | tr '\n' ' ')
echo "$plane" | awk '{ split("0 0 5 1 0 0 0 1 0", w, " "); ok = (NF == 9); for (i = 1; i <= 9; i++) { d = $i - w[i]; if (d < 0) d = -d; if (d > 1e-9) ok = 0 } exit !ok }' || bad "sketch 1 plane is [$plane], expected origin 0 0 5, x 1 0 0, y 0 1 0"
# 3. circle
flat 24 | grep -q '"name":"Circle[0-9]*","shape":{"Circle":{[^}]*}},"sketch":1' || bad "the circle is not in sketch 1"
# 4. blind hole
v4=$(volume 33)
near "$v4" "$(awk 'BEGIN {print 4000 - 3.141592653589793 * 16 * 2}')" 0.003 || bad "volume after the blind hole is $v4"
awk -v v="$v4" 'BEGIN {exit !(v < 4000)}' || bad "volume after the blind hole is not below 4000"
# 5. second body
[ "$(field 37 sketch)" = 2 ] || bad "active sketch after Add sketch is $(field 37 sketch), expected 2"
[ "$(nbodies 50)" = 2 ] || bad "expected 2 bodies, got $(nbodies 50)"
near "$(volume 50)" "$(awk -v v="$v4" 'BEGIN {print v + 500}')" 1e-6 || bad "total volume with the new body is $(volume 50), expected $v4 + 500"

[ "$fail" -eq 0 ] && echo "MCP SKETCH PASS" || { echo "outputs in $OUT"; exit 1; }
