#!/usr/bin/env bash
# End-to-end test of section caps, edge selection and the translate gizmo through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase3b.out
rm -f "$OUT" target/mcp_phase3b_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase3b","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. box 40 x 20 x 10, then fit
  cw 3 Rect
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":40,"y":20,"world":true}'
  cw 6 Model
  call 7 click '{"x":20,"y":0,"world":true}'
  st 8 Distance "10 mm"
  cw 9 Extrude
  call 10 state '{}'
  cw 11 3D
  call 12 key '{"key":"F"}'
  # 2. section at x = 20: the cap is the 20 x 10 cut
  cw 13 "Section view"
  st 14 "Section offset" "20"
  cw 15 "Section axis"
  cw 16 X
  call 17 state '{}'
  call 18 screenshot '{}'
  cw 19 "Section view"
  call 20 screenshot '{}'
  # 3. click on the front top edge of the body: pixel (538, 411) in the fitted 1280 x 800 window, read from the screenshot of request 20
  call 21 click '{"x":538,"y":411}'
  call 22 state '{}'
  # 4. add a 20 mm box; its gizmo is at [[282,334],[425,377],...] (origin, X, Y, Z tips). Drag from the middle of the X arrow one arrow length along it
  cw 23 Box
  call 24 state '{}'
  call 25 drag '{"x1":354,"y1":356,"x2":497,"y2":399,"button":"primary"}'
  call 26 state '{}'
} | target/debug/cad-app.exe --mcp > "$OUT" 2> target/mcp_phase3b.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }

[ "$(grep -c . "$OUT")" -ge 25 ] || bad "expected 25 responses, got $(grep -c . "$OUT")"
area=$(flat 17 | grep -oE '"section_cap_area":[0-9.]+' | cut -d: -f2)
awk -v a="${area:-0}" 'BEGIN { d = a - 200; exit !(d < 0.01 && d > -0.01) }' || bad "section_cap_area is '$area', expected 200"
grep '"id":18,' "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase3b_3d.png 2>/dev/null
[ "$(head -c 4 target/mcp_phase3b_3d.png | tail -c 3)" = "PNG" ] || bad "3D screenshot is not a PNG"
grep '"id":20,' "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase3b_edge.png 2>/dev/null
flat 22 | grep -q '"edge_sel":1' || bad "no edge selected: $(flat 22 | grep -o '"edge_sel":[^,]*')"
flat 22 | grep -q 'Edge:line,length' || bad "status is not an edge message"
flat 22 | grep -q 'convex' || bad "edge is not convex"
flat 24 | grep -q '"gizmo":\[\[' || bad "no gizmo after adding a box"
box=$(flat 26 | grep -oE '"Box":\{"center":\[[-0-9.e]+,[-0-9.e]+,[-0-9.e]+\]' | tail -1)
cx=$(echo "$box" | sed -E 's/.*\[([-0-9.e]+),.*/\1/')
[ "$(echo "$box" | grep -c ',0.0,10.0\]')" -eq 1 ] || bad "box moved off its axis: $box"
awk -v x="${cx:-0}" 'BEGIN { exit !(x > 1) }' || bad "box centre x is '$cx', expected > 1"

[ "$fail" -eq 0 ] && echo "MCP PHASE3B PASS" || { echo "outputs in $OUT"; exit 1; }
