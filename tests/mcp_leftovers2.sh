#!/usr/bin/env bash
# End-to-end test of associative dimensions, groups, metadata, PDF export and SVG import through `cad-app --mcp` only.
# No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_leftovers2.out
PDF=target/mcp_l2.pdf
SVG=target/mcp_l2.svg
rm -f "$OUT" "$PDF" "$SVG"

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"leftovers2","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. a line, a circle, and a dimension of the line's end points
  cw 3 Line
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":40,"y":0,"world":true}'
  cw 6 Circle
  call 7 click '{"x":60,"y":0,"world":true}'
  call 8 click '{"x":65,"y":0,"world":true}'
  cw 9 Dimension
  call 10 click '{"x":0,"y":0,"world":true}'
  call 11 click '{"x":40,"y":0,"world":true}'
  call 12 click '{"x":20,"y":15,"world":true}'
  call 13 state '{}'
  # 2. moving the line's end moves the dimension; one undo restores both
  cw 14 Select
  call 15 click '{"x":20,"y":0,"world":true}'
  st 16 "b x" 60
  call 17 state '{}'
  call 18 key '{"key":"Z","ctrl":true}'
  call 19 state '{}'
  # 3. group the line and the circle, select the group by clicking the circle, ungroup
  call 20 click '{"x":20,"y":0,"world":true}'
  call 21 click '{"x":65,"y":0,"world":true,"shift":true}'
  cw 22 Group
  call 23 state '{}'
  call 24 click '{"x":300,"y":700}'
  call 25 click '{"x":65,"y":0,"world":true}'
  call 26 state '{}'
  cw 27 Ungroup
  call 28 state '{}'
  # 4. metadata of the circle
  call 29 click '{"x":65,"y":0,"world":true}'
  st 30 "Meta key" material
  st 31 "Meta value" steel
  cw 32 "Add meta"
  call 33 state '{}'
  # 5. export PDF and SVG, start over, import the SVG
  call 34 export_project "{\"path\":\"$PDF\"}"
  call 35 export_project "{\"path\":\"$SVG\"}"
  call 36 new_project '{}'
  call 37 import_project "{\"path\":\"$SVG\"}"
  call 38 state '{}'
} | target/debug/cad-app.exe --mcp > "$OUT" 2> target/mcp_leftovers2.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
near() { awk -v a="$1" -v b="$2" -v t="${3:-1e-6}" 'BEGIN { d = a - b; exit !(d < t && d > -t) }'; }
same() { awk -v s="$1" -v w="$2" -v t="${3:-1e-6}" 'BEGIN { n = split(s, a, " "); m = split(w, b, " "); if (n != m || n == 0) exit 1; for (i = 1; i <= n; i++) { d = a[i] - b[i]; if (d > t || d < -t) exit 1 } }'; }
# numbers of FIELD of the Nth shape object KIND in state ID: shape ID KIND FIELD [N]
shape() {
  flat "$1" | grep -oE "\"$2\":\{[^}]*\}" | sed -n "${4:-1}p" | grep -oE "\"$3\":(\[[^]]*\]|[-0-9.e]+|true|false)" \
    | sed -E 's/^"[a-z_]+"://' | tr -d '[]' | tr ',' ' '
}
count() { flat "$1" | grep -oE "$2" | wc -l; }

[ "$(grep -c . "$OUT")" -ge 37 ] || bad "expected 37 responses, got $(grep -c . "$OUT")"
for id in 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38; do
  [ -n "$(flat $id)" ] && ! flat $id | grep -q '"isError":true' || bad "call $id failed"
done
# 1. the dimension (entity 3) is linked to the start and end of line 1
[ "$(count 13 '"Dimension":\{')" -eq 1 ] || bad "expected one Dimension"
flat 13 | grep -q '"dim_links":{"3":\[{"entity":1,"kind":"Start"},{"entity":1,"kind":"End"}\]}' || bad "dim_links of the dimension: $(flat 13 | grep -oE '"dim_links":[^}]*}[^}]*}' | head -1)"
# 2. b x = 60 moves the dimension's b; the undo restores both
same "$(shape 17 Line b)" "60 0" || bad "line b after the edit is $(shape 17 Line b)"
same "$(shape 17 Dimension b)" "60 0" || bad "dimension b did not follow: $(shape 17 Dimension b)"
same "$(shape 19 Line b)" "40 0" || bad "line b after undo is $(shape 19 Line b)"
same "$(shape 19 Dimension b)" "40 0" || bad "dimension b after undo is $(shape 19 Dimension b)"
# 3. the line and the circle share group 1, the dimension has none; a click on the circle selects both; Ungroup clears it
[ "$(count 23 '"group":1,')" -eq 2 ] && [ "$(count 23 '"group":null')" -eq 1 ] || bad "groups after Group: $(count 23 '"group":1,') in group 1, $(count 23 '"group":null') without"
flat 26 | grep -q '"selection":\[1,2\]' || bad "clicking the circle selected $(flat 26 | grep -oE '"selection":\[[0-9,]*\]' | head -1)"
[ "$(count 28 '"group":null')" -eq 3 ] || bad "Ungroup left a group: $(count 28 '"group":null') entities without one"
# 4. metadata
flat 33 | grep -q '"meta":{"material":"steel"}' || bad "metadata of the circle is missing"
# 5. PDF and SVG files; the SVG imports back
[ "$(head -c 8 "$PDF")" = "%PDF-1.4" ] || bad "pdf does not start with %PDF-1.4"
[ "$(tail -c 5 "$PDF")" = "%%EOF" ] || bad "pdf does not end with %%EOF"
grep -aq '^xref' "$PDF" && grep -aq ' m ' "$PDF" || bad "pdf has no xref table or no path"
grep -q '<circle' "$SVG" || bad "svg has no circle"
flat 37 | grep -qE '"status":"Imported[0-9]+entities' || bad "import reply: $(flat 37 | cut -c1-200)"
same "$(shape 38 Line a 1) $(shape 38 Line b 1)" "0 0 40 0" || bad "imported line is $(shape 38 Line a 1) $(shape 38 Line b 1)"
near "$(shape 38 Circle radius)" 5 || bad "imported circle radius $(shape 38 Circle radius)"
same "$(shape 38 Circle center)" "60 0" || bad "imported circle centre $(shape 38 Circle center)"

[ "$fail" -eq 0 ] && echo "MCP LEFTOVERS2 PASS" || { echo "outputs in $OUT"; exit 1; }
