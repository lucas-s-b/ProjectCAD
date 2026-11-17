#!/usr/bin/env bash
# End-to-end test of STEP export with analytic faces (a plate with a coplanar cut hole) through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase3c.out
STEP="$(pwd -W 2>/dev/null || pwd)/target/mcp_phase3c.step"
rm -f "$OUT" "$STEP"

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase3c","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. rectangle and circle
  cw 3 Rect
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":40,"y":20,"world":true}'
  cw 6 Circle
  call 7 click '{"x":20,"y":10,"world":true}'
  call 8 click '{"x":24,"y":10,"world":true}'
  call 9 key '{"key":"Escape"}'
  # 2. extrude the rectangle by 10 mm
  cw 10 Model
  call 11 click '{"x":20,"y":0,"world":true}'
  st 12 Distance "10 mm"
  cw 13 Extrude
  call 14 state '{}'
  # 3. cut the circle (the Distance field keeps 10 mm)
  call 15 click '{"x":24,"y":10,"world":true}'
  cw 16 Operation
  cw 17 Cut
  cw 18 Extrude
  call 19 state '{}'
  # 4. STEP export
  call 20 export_project "{\"path\":\"$STEP\"}"
} | target/debug/cad-app.exe --mcp > "$OUT" 2> target/mcp_phase3c.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
count() { grep -c "=$1(" "$STEP" 2>/dev/null; }

[ "$(grep -c . "$OUT")" -ge 19 ] || bad "expected 19 responses, got $(grep -c . "$OUT")"
[ "$(head -n 1 "$STEP" 2>/dev/null | tr -d '\r')" = "ISO-10303-21;" ] || bad "STEP file missing or wrong first line"
for want in ADVANCED_FACE:7 CYLINDRICAL_SURFACE:1 CIRCLE:2 MANIFOLD_SOLID_BREP:1 FACETED_BREP:0; do
  [ "$(count "${want%%:*}")" = "${want##*:}" ] || bad "${want%%:*}: got $(count "${want%%:*}"), expected ${want##*:}"
done
[ "$(grep -c 'CYLINDRICAL_SURFACE(.*,4.000000)' "$STEP" 2>/dev/null)" = 1 ] || bad "no cylinder of radius 4"

[ "$fail" -eq 0 ] && echo "MCP PHASE3C PASS" || { echo "outputs in $OUT"; exit 1; }
