#!/usr/bin/env bash
# End-to-end test of edge picking, fillets and chamfers through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_blend.out
rm -f "$OUT" target/mcp_blend_*.png target/mcp_box.stl target/mcp_box.obj

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }

# Picks in the 3D view are pixels of the 1280x800 screenshot, taken after F: the vertical edge at the origin corner
# (292,390); the far long edge of the plate (660,302); empty space (700,720). Everything else is clicked by widget name.
{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"blend","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. rectangle, extruded by 10 mm
  cw 3 Rect
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":40,"y":20,"world":true}'
  call 6 key '{"key":"Escape"}'
  cw 7 Model
  call 8 click '{"x":20,"y":0,"world":true}'
  cw 9 Distance
  call 10 key '{"key":"A","ctrl":true}'
  call 11 type_text '{"text":"10 mm"}'
  cw 12 Extrude
  call 13 state '{}'
  call 50 export_project '{"path":"target/mcp_box.stl"}'
  call 51 export_project '{"path":"target/mcp_box.obj"}'
  # 2. fillet the vertical edge at the origin: pick, unpick, pick, Enter
  cw 14 Size
  call 15 key '{"key":"A","ctrl":true}'
  call 16 type_text '{"text":"3 mm"}'
  cw 17 3D
  cw 18 "Fillet edges"
  call 19 state '{}'
  call 20 key '{"key":"F"}'
  call 21 screenshot '{}'
  call 22 click '{"x":292,"y":390}'
  call 23 state '{}'
  call 24 click '{"x":292,"y":390}'
  call 25 state '{}'
  call 26 click '{"x":292,"y":390}'
  call 27 state '{}'
  call 28 key '{"key":"Enter"}'
  call 29 state '{}'
  # 3. chamfer the far long edge
  cw 30 Size
  call 31 key '{"key":"A","ctrl":true}'
  call 32 type_text '{"text":"2 mm"}'
  cw 33 "Chamfer edges"
  call 34 screenshot '{}'
  call 35 click '{"x":660,"y":302}'
  call 36 state '{}'
  call 37 key '{"key":"Enter"}'
  call 38 state '{}'
  # 4. look at the result
  call 39 screenshot '{}'
  # 5. undo both
  call 40 key '{"key":"Z","ctrl":true}'
  call 41 key '{"key":"Z","ctrl":true}'
  call 42 state '{}'
  # 6. nothing under the click, then Escape
  cw 43 "Fillet edges"
  call 44 click '{"x":700,"y":720}'
  call 45 state '{}'
  call 46 key '{"key":"Escape"}'
  call 47 state '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_blend.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
volume() { flat "$1" | grep -oE '"volume":[-0-9.e+]+' | head -1 | cut -d: -f2; }
states() { flat "$1" | grep -oE '"feature_states":\{[^}]*\}'; }
picks() { flat "$1" | grep -oE '"pick_edges":[^,]*' | cut -d: -f2; }
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }

[ "$(grep -c . "$OUT")" -ge 48 ] || bad "expected 48 responses, got $(grep -c . "$OUT")"
for n in 21 34 39; do
  grep "\"id\":$n," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_blend_$n.png 2>/dev/null
  [ "$(head -c 4 target/mcp_blend_$n.png | tail -c 3)" = "PNG" ] || bad "screenshot $n is not a PNG"
done

# 1. box
near "$(volume 13)" 8000 1e-6 || bad "box volume is $(volume 13), expected 8000"
flat 13 | grep -q '"edges":12' || bad "box does not have 12 edges"
# STL and OBJ of the box: 12 triangles, 8 shared vertices
[ "$(wc -c < target/mcp_box.stl)" -eq $((84 + 50 * 12)) ] || bad "STL size is $(wc -c < target/mcp_box.stl), expected $((84 + 50 * 12))"
[ "$(grep -c '^v ' target/mcp_box.obj)" -eq 8 ] || bad "OBJ has $(grep -c '^v ' target/mcp_box.obj) v lines, expected 8"
[ "$(grep -c '^f ' target/mcp_box.obj)" -eq 12 ] || bad "OBJ has $(grep -c '^f ' target/mcp_box.obj) f lines, expected 12"
# 2. fillet
[ "$(picks 19)" = 0 ] || bad "pick_edges after Fillet edges is $(picks 19), expected 0"
flat 19 | grep -q '"view":"ThreeD"' || bad "view is not 3D"
[ "$(picks 23)" = 1 ] || bad "pick_edges after the first click is $(picks 23), expected 1"
[ "$(picks 25)" = 0 ] || bad "pick_edges after the second click is $(picks 25), expected 0"
[ "$(picks 27)" = 1 ] || bad "pick_edges after the third click is $(picks 27), expected 1"
states 29 | grep -q '{"1":"Ok","2":"Ok"}' || bad "states after the fillet are $(states 29)"
[ "$(picks 29)" = null ] || bad "still picking after Enter"
v2=$(awk 'BEGIN {print 8000 - (9 - 9 * 3.141592653589793 / 4) * 10}')
near "$(volume 29)" "$v2" 0.1 || bad "volume after the fillet is $(volume 29), expected $v2"
# 3. chamfer
states 38 | grep -q '{"1":"Ok","2":"Ok","3":"Ok"}' || bad "states after the chamfer are $(states 38)"
near "$(volume 38)" "$(awk -v v="$(volume 29)" 'BEGIN {print v - 80}')" 0.5 || bad "volume after the chamfer is $(volume 38), expected $(volume 29) - 80"
# 5. undo
near "$(volume 42)" 8000 1e-6 || bad "volume after the undos is $(volume 42), expected 8000"
states 42 | grep -q '{"1":"Ok"}' || bad "states after the undos are $(states 42)"
# 6. no edge
flat 45 | grep -q 'Noedgethere' || bad "no 'No edge there' status"
[ "$(picks 47)" = null ] || bad "still picking after Escape"

[ "$fail" -eq 0 ] && echo "MCP BLEND PASS" || { echo "outputs in $OUT"; exit 1; }
