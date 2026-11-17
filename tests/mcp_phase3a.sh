#!/usr/bin/env bash
# End-to-end test of face selection, the section plane and STEP export through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase3a.out
STEP="$(pwd -W 2>/dev/null || pwd)/target/mcp_phase3a.step"
rm -f "$OUT" "$STEP" target/mcp_phase3a_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase3a","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. box 40 x 20 x 10
  cw 3 Rect
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":40,"y":20,"world":true}'
  cw 6 Model
  call 7 click '{"x":20,"y":0,"world":true}'
  st 8 Distance "10 mm"
  cw 9 Extrude
  call 10 state '{}'
  # 2. fit, then a click on the top face (pixel (600, 356) in the fitted 1280 x 800 window) selects it
  cw 11 3D
  call 12 key '{"key":"F"}'
  call 13 click '{"x":600,"y":356}'
  call 14 state '{}'
  # 3. section at x = 20
  cw 15 "Section view"
  st 16 "Section offset" "20"
  cw 17 "Section axis"
  cw 18 X
  call 19 state '{}'
  call 20 screenshot '{}'
  # 4. STEP export
  call 21 export_project "{\"path\":\"$STEP\"}"
} | target/debug/cad-app.exe --mcp > "$OUT" 2> target/mcp_phase3a.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }

[ "$(grep -c . "$OUT")" -ge 20 ] || bad "expected 20 responses, got $(grep -c . "$OUT")"
flat 14 | grep -q '"face_sel":\[1,' || bad "no face selected: $(flat 14 | grep -o '"face_sel":[^]]*]')"
flat 14 | grep -q 'Face:plane,area' || bad "status is not a face message"
flat 19 | grep -q '"section":\[1.0,0.0,0.0,20.0\]' || bad "section is $(flat 19 | grep -oE '"section":[^]]*]')"
grep '"id":20,' "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase3a_3d.png 2>/dev/null
[ "$(head -c 4 target/mcp_phase3a_3d.png | tail -c 3)" = "PNG" ] || bad "3D screenshot is not a PNG"
[ "$(head -n 1 "$STEP" 2>/dev/null | tr -d '\r')" = "ISO-10303-21;" ] || bad "STEP file missing or wrong first line"
[ "$(grep -c FACE_SURFACE "$STEP" 2>/dev/null)" -gt 0 ] || bad "STEP file has no faces"

[ "$fail" -eq 0 ] && echo "MCP PHASE3A PASS" || { echo "outputs in $OUT"; exit 1; }
