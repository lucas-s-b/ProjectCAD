#!/usr/bin/env bash
# End-to-end test of a chamfer edge and a sketch face that follow a taller plate, through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase3d.out
rm -f "$OUT" target/mcp_phase3d_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

# Picks in the 3D view are pixels of the 1280x800 screenshot, taken after F: the far long edge of the plate (660,302)
# and the middle of its top face (600,356). Everything else is clicked by widget name.
{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase3d","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. rectangle; parameter h = 10 mm; extrude the rectangle by h
  cw 3 Rect
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":40,"y":20,"world":true}'
  call 6 key '{"key":"Escape"}'
  cw 7 Model
  cw 8 "Param name"
  call 9 type_text '{"text":"h"}'
  cw 10 "Param expression"
  call 11 type_text '{"text":"10 mm"}'
  cw 12 Add
  call 13 click '{"x":20,"y":0,"world":true}'
  st 14 Distance "h"
  cw 15 Extrude
  call 16 state '{}'
  # 2. chamfer the far long edge (pixel (660,302), as in tests/mcp_blend.sh)
  cw 17 Size
  call 18 key '{"key":"A","ctrl":true}'
  call 19 type_text '{"text":"2 mm"}'
  cw 20 3D
  cw 21 "Chamfer edges"
  call 22 key '{"key":"F"}'
  call 23 screenshot '{}'
  call 24 click '{"x":660,"y":302}'
  call 25 key '{"key":"Enter"}'
  call 26 state '{}'
  # 3. sketch on the top face (pixel (600,356), as in tests/mcp_shell.sh), circle r 4, boss 3 mm
  cw 27 "Sketch on face"
  call 28 key '{"key":"F"}'
  call 29 screenshot '{}'
  call 30 click '{"x":600,"y":356}'
  call 31 state '{}'
  cw 32 Circle
  call 33 click '{"x":20,"y":10,"world":true}'
  call 34 click '{"x":24,"y":10,"world":true}'
  call 35 key '{"key":"Escape"}'
  call 36 click '{"x":24,"y":10,"world":true}'
  cw 37 Distance
  call 38 key '{"key":"A","ctrl":true}'
  call 39 type_text '{"text":"3 mm"}'
  cw 40 Extrude
  call 41 state '{}'
  # 4. h = 15 mm: the chamfer and the boss follow the top face
  cw 42 "Param h"
  call 43 key '{"key":"A","ctrl":true}'
  call 44 type_text '{"text":"15 mm"}'
  call 45 key '{"key":"Enter"}'
  call 46 state '{}'
} | target/debug/cad-app.exe --mcp > "$OUT" 2> target/mcp_phase3d.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
volume() { flat "$1" | grep -oE '"volume":[-0-9.e+]+' | head -1 | cut -d: -f2; }
states() { flat "$1" | grep -oE '"feature_states":\{[^}]*\}'; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }

[ "$(grep -c . "$OUT")" -ge 45 ] || bad "expected at least 45 responses, got $(grep -c . "$OUT")"
for n in 23 29; do
  grep "\"id\":$n," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase3d_$n.png 2>/dev/null
  [ "$(head -c 4 target/mcp_phase3d_$n.png | tail -c 3)" = "PNG" ] || bad "screenshot $n is not a PNG"
done

# 1. plate
near "$(volume 16)" 8000 1e-6 || bad "plate volume is $(volume 16), expected 8000"
# 2. chamfer: 8000 - 80
states 26 | grep -q '{"1":"Ok","2":"Ok"}' || bad "states after the chamfer are $(states 26)"
near "$(volume 26)" 7920 0.5 || bad "volume after the chamfer is $(volume 26), expected 7920"
# 3. sketch on the top face, then a boss: 7920 + 3 * area of the faceted circle
flat 31 | grep -q '"plane":{"Face":{[^}]*"body":1[,}]' || bad "sketch 1 is not on a face of body 1"
states 41 | grep -q '{"1":"Ok","2":"Ok","3":"Ok"}' || bad "states after the boss are $(states 41)"
near "$(volume 41)" 8070.8 2 || bad "volume after the boss is $(volume 41), expected 8070.8"
# 4. h = 15 mm: 12000 - 80 + 48 pi
states 46 | grep -q '{"1":"Ok","2":"Ok","3":"Ok"}' || bad "states after h = 15 mm are $(states 46)"
near "$(volume 46)" 12070.8 2 || bad "volume after h = 15 mm is $(volume 46), expected 12070.8"

[ "$fail" -eq 0 ] && echo "MCP PHASE3D PASS" || { echo "outputs in $OUT"; exit 1; }
