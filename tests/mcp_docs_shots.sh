#!/usr/bin/env bash
# Screenshots for docs/USER_GUIDE.md and README.md, taken through `cad-app --mcp --background` only. No OS input.
# Writes to target/docs_shots; `SHOTS=docs/images bash tests/mcp_docs_shots.sh` remakes the images in the docs.
cd "$(dirname "$0")/.." || exit 1
SHOTS=${SHOTS:-target/docs_shots}
OUT=target/mcp_docs_shots.out
HOUSE=target/mcp_docs_house.cadproj
GEARS=target/mcp_docs_gears.cadproj
BEAM=target/mcp_docs_beam.cadproj
rm -f "$OUT" "$HOUSE" "$GEARS" "$BEAM" target/mcp_docs_shots.err
mkdir -p "$SHOTS"

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

target/debug/cad-bench.exe house "$HOUSE" || { echo "cannot write $HOUSE"; exit 1; }
target/debug/cad-bench.exe assembly "$GEARS" 3 100 || { echo "cannot write $GEARS"; exit 1; }
target/debug/cad-bench.exe beam "$BEAM" || { echo "cannot write $BEAM"; exit 1; }
# init <client name>: the initialize request and notification of one app launch
init() { printf '%s\n' "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"$1\",\"version\":\"0\"}}}" '{"jsonrpc":"2.0","method":"notifications/initialized"}'; }

# ids of the click_widget calls and of the screenshot calls below
CW_IDS="3 8 13 23 26 30 31 33 35 38 41 44 45 46 48 53 57 61 65 66 70 71 73 76 77"
SHOT_IDS="drafting:16 parametric:59 architecture:63 assembly:68 analysis:75 command-search:80"

{ init docs_shots
  # 1. drafting (the calls of mcp_drafting.sh up to its screenshot)
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
  # 2. parametric part (the calls of mcp_model.sh up to its screenshot)
  call 20 new_project '{}'
  cw 23 Rect
  call 24 click '{"x":0,"y":0,"world":true}'
  call 25 click '{"x":40,"y":20,"world":true}'
  cw 26 Circle
  call 27 click '{"x":20,"y":10,"world":true}'
  call 28 click '{"x":24,"y":10,"world":true}'
  call 29 key '{"key":"Escape"}'
  cw 30 Model
  cw 31 "Param name"
  call 32 type_text '{"text":"t"}'
  cw 33 "Param expression"
  call 34 type_text '{"text":"5 mm"}'
  cw 35 Add
  call 36 state '{}'
  call 37 click '{"x":20,"y":0,"world":true}'
  cw 38 Distance
  call 39 key '{"key":"A","ctrl":true}'
  call 40 type_text '{"text":"t"}'
  cw 41 Extrude
  call 42 state '{}'
  call 43 click '{"x":24,"y":10,"world":true}'
  cw 44 Operation
  cw 45 Cut
  cw 46 Extrude
  call 47 state '{}'
  cw 48 "Param t"
  call 49 key '{"key":"A","ctrl":true}'
  call 50 type_text '{"text":"10 mm"}'
  call 51 key '{"key":"Enter"}'
  call 52 state '{}'
  cw 53 S 1
  call 54 state '{}'
  call 55 key '{"key":"Z","ctrl":true}'
  call 56 state '{}'
  cw 57 3D
  call 58 key '{"key":"F"}'
  call 59 screenshot '{}'
  # 3. the house in 3D
  call 60 open_project "{\"path\":\"$HOUSE\"}"
  cw 61 3D
  call 62 key '{"key":"F"}'
  call 63 screenshot '{}'
  # 4. gear components
  call 64 open_project "{\"path\":\"$GEARS\"}"
  cw 65 Assembly
  cw 66 3D
  call 67 key '{"key":"F"}'
  call 68 screenshot '{}'
  # 5. the beam with its static study
  call 69 open_project "{\"path\":\"$BEAM\"}"
  cw 70 Model
  cw 71 "Run static study"
  call 72 state '{}'
  cw 73 3D
  call 74 key '{"key":"F"}'
  call 75 screenshot '{}'
  # 6. light theme and command search (not saved: no CAD_SETTINGS in MCP mode)
  cw 76 View
  cw 77 "Light theme"
  call 78 key '{"key":"K","ctrl":true}'
  call 79 type_text '{"text":"run"}'
  call 80 screenshot '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_docs_shots.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }

# 73 calls above plus 1 initialize
[ "$(grep -c . "$OUT")" -ge 74 ] || bad "expected at least 74 responses, got $(grep -c . "$OUT")"
for id in $CW_IDS; do
  [ -n "$(grep "\"id\":$id," "$OUT")" ] && ! grep "\"id\":$id," "$OUT" | grep -q '"isError":true' || bad "click_widget $id failed: $(grep "\"id\":$id," "$OUT" | cut -c1-200)"
done
for s in $SHOT_IDS; do
  name=${s%%:*}
  id=${s##*:}
  f="$SHOTS/$name.png"
  grep "\"id\":$id," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > "$f" 2>/dev/null
  [ "$(head -c 4 "$f" | tail -c 3)" = "PNG" ] || bad "$name.png is not a PNG"
  [ "$(wc -c < "$f")" -lt 409600 ] || bad "$name.png is 400 KB or more"
done

[ "$fail" -eq 0 ] && echo "MCP DOCS SHOTS PASS" || { echo "outputs in $OUT"; exit 1; }
