#!/usr/bin/env bash
# End-to-end test of text, dimension and export through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_drafting.out
SVG=target/mcp_test.svg
DXF=target/mcp_test.dxf
SHOT=target/mcp_drafting.png
rm -f "$OUT" "$SVG" "$DXF" "$SHOT" target/x.xyz

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"drafting","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  cw 3 Line
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":100,"y":0,"world":true}'
  call 6 screenshot '{}'
  call 7 state '{}'
  cw 8 Dimension
  call 9 click '{"x":0,"y":0,"world":true}'
  call 10 click '{"x":100,"y":0,"world":true}'
  call 11 click '{"x":50,"y":20,"world":true}'
  call 12 state '{}'
  cw 13 Text
  call 14 click '{"x":10,"y":-20,"world":true}'
  call 15 state '{}'
  call 16 screenshot '{}'
  call 17 export_project "{\"path\":\"$SVG\"}"
  call 18 export_project "{\"path\":\"$DXF\"}"
  call 19 export_project '{"path":"target/x.xyz"}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_drafting.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
near() { awk -v a="$1" -v b="$2" 'BEGIN { d = a - b; exit !(d < 1e-6 && d > -1e-6) }'; }
# the numbers of one JSON field, e.g. field 12 Dimension a -> "0.0 0.0"
nums() { grep -oE "\"$3\":(\[[^]]*\]|[-0-9.e]+)" <<< "$(flat "$1" | grep -oE "\"$2\":\{[^}]*\}")" | sed -E 's/^"[a-z_]+"://' | tr -d '[]' | tr ',' ' '; }

[ "$(grep -c . "$OUT")" -ge 18 ] || bad "expected 18 responses, got $(grep -c . "$OUT")"
for id in 3 8 13 17 18; do
  [ -n "$(flat $id)" ] && ! flat $id | grep -q '"isError":true' || bad "call $id failed"
done
flat 3 | grep -q '"tool":"Line"' || bad "Line tool not selected"
flat 7 | grep -q '"Line":{' || bad "no Line after drawing"
flat 8 | grep -q '"tool":"Dimension"' || bad "Dimension tool not selected"
[ "$(flat 12 | grep -o '"Dimension":{' | wc -l)" -eq 1 ] || bad "expected 1 Dimension"
read -r ax ay <<< "$(nums 12 Dimension a)"
read -r bx by <<< "$(nums 12 Dimension b)"
{ near "$ax" 0 && near "$ay" 0 && near "$bx" 100 && near "$by" 0; } || bad "dimension points $ax $ay $bx $by"
near "$(nums 12 Dimension offset)" 20 || bad "dimension offset $(nums 12 Dimension offset)"
flat 13 | grep -q '"tool":"Text"' || bad "Text tool not selected"
[ "$(flat 15 | grep -o '"Text":{' | wc -l)" -eq 1 ] || bad "expected 1 Text"
read -r px py <<< "$(nums 15 Text pos)"
{ near "$px" 10 && near "$py" -20; } || bad "text position $px $py"
near "$(nums 15 Text height)" 5 || bad "text height"
flat 15 | grep -q '"text":"Text"' || bad "text content"
flat 19 | grep -q '"isError":true' || bad "export to an unknown format should fail"
flat 19 | grep -q 'unknownexportformat' || bad "unknown format error text"
[ -f "$SVG" ] && grep -q '<text' "$SVG" && grep -q '100.00' "$SVG" || bad "svg lacks <text or the value 100.00"
[ -f "$DXF" ] && grep -qx 'TEXT' "$DXF" && [ "$(tail -n 1 "$DXF")" = "EOF" ] || bad "dxf lacks TEXT or EOF"
grep '"id":16,' "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > "$SHOT" 2>/dev/null
[ "$(head -c 4 "$SHOT" | tail -c 3)" = "PNG" ] || bad "screenshot is not a PNG"

[ "$fail" -eq 0 ] && echo "MCP DRAFTING PASS" || { echo "outputs in $OUT"; exit 1; }
