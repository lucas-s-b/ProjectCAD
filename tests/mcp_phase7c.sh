#!/usr/bin/env bash
# End-to-end test of glTF, 3MF, IFC and STEP export and STL/OBJ import, through `cad-app --mcp` only. No OS input.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase7c.out
GEARS=target/mcp_phase7c_gears.cadproj
HOUSE=target/mcp_phase7c_house.cadproj
GLB=target/mcp_phase7c.glb
TMF=target/mcp_phase7c.3mf
STL=target/mcp_phase7c.stl
OBJ=target/mcp_phase7c.obj
IFC=target/mcp_phase7c.ifc
SAVED=target/mcp_phase7c_saved.cadproj
OPEN=target/mcp_phase7c_open.stl
rm -f "$OUT" "$GEARS" "$HOUSE" "$GLB" "$TMF" "$STL" "$OBJ" "$IFC" "$SAVED" "$OPEN"

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

target/debug/cad-bench.exe assembly "$GEARS" 3 100 || { echo "cannot write $GEARS"; exit 1; }
target/debug/cad-bench.exe house "$HOUSE" || { echo "cannot write $HOUSE"; exit 1; }
# an open mesh with four triangles: a pyramid without its base
{
  echo 'solid t'
  for f in '0 0 0|1 0 0|0.5 0.5 1' '1 0 0|1 1 0|0.5 0.5 1' '1 1 0|0 1 0|0.5 0.5 1' '0 1 0|0 0 0|0.5 0.5 1'; do
    printf 'facet normal 0 0 0\nouter loop\n'
    IFS='|' read -r a b c <<< "$f"
    printf 'vertex %s\nvertex %s\nvertex %s\nendloop\nendfacet\n' "$a" "$b" "$c"
  done
  echo 'endsolid t'
} > "$OPEN"

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase7c","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. three gears of one body: one shared mesh in glTF and 3MF
  call 3 open_project "{\"path\":\"$GEARS\"}"
  call 4 state '{}'
  call 5 export_project "{\"path\":\"$GLB\"}"
  call 6 export_project "{\"path\":\"$TMF\"}"
  call 7 export_project "{\"path\":\"$STL\"}"
  call 8 export_project "{\"path\":\"$OBJ\"}"
  # 2. the STL and OBJ come back as mesh bodies, survive save and open
  call 9 new_project '{}'
  call 10 import_project "{\"path\":\"$STL\"}"
  call 11 state '{}'
  call 12 import_project "{\"path\":\"$OBJ\"}"
  call 13 state '{}'
  call 14 save_project "{\"path\":\"$SAVED\"}"
  call 15 open_project "{\"path\":\"$SAVED\"}"
  call 16 state '{}'
  # 3. an open mesh is refused
  call 17 import_project "{\"path\":\"$OPEN\"}"
  # 4. the sample house as IFC
  call 18 open_project "{\"path\":\"$HOUSE\"}"
  call 19 export_project "{\"path\":\"$IFC\"}"
  call 20 state '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase7c.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# number field $1 of the row on stdin
num() { grep -oE "\"$1\":[-0-9.e]+" | head -1 | cut -d: -f2; }
# $1 >= $2
ge() { awk -v a="$1" -v b="$2" 'BEGIN {exit !(a != "" && a >= b)}'; }
# the bodies array of state $1
bv() { flat "$1" | grep -oE '"bodies":\[[^]]*\]'; }
# volume number $2 (1-based) of the bodies of state $1
vol() { bv "$1" | grep -oE '"volume":[-0-9.e]+' | sed -n "${2}p" | cut -d: -f2; }
cnt() { grep -ao "$1" "$2" | wc -l; }

[ "$(grep -c . "$OUT")" -ge 19 ] || bad "expected at least 19 responses, got $(grep -c . "$OUT")"

# glTF: one shared mesh, three nodes
[ "$(head -c 4 "$GLB")" = glTF ] || bad "glb magic"
[ "$(cnt '"primitives"' "$GLB")" -eq 1 ] || bad "glb meshes: $(cnt '"primitives"' "$GLB")"
[ "$(cnt '"mesh":0' "$GLB")" -eq 3 ] || bad "glb nodes: $(cnt '"mesh":0' "$GLB")"
# 3MF: one object, three build items
[ "$(head -c 2 "$TMF")" = PK ] || bad "3mf magic"
grep -aq '3D/3dmodel.model' "$TMF" || bad "3mf model part"
[ "$(cnt '<object ' "$TMF")" -eq 1 ] || bad "3mf objects: $(cnt '<object ' "$TMF")"
[ "$(cnt '<item ' "$TMF")" -eq 3 ] || bad "3mf items: $(cnt '<item ' "$TMF")"
# the STL of the three gears comes back as mesh bodies of three times the volume
V=$(vol 4 1)
TOL=$(awk -v v="$V" 'BEGIN {print 3 * v * 1e-4}')
W3=$(awk -v v="$V" 'BEGIN {print 3 * v}')
near "$(vol 11 1)" "$W3" "$TOL" || bad "STL import volume: $(vol 11 1) vs $W3 ($(bv 11))"
flat 11 | grep -qF '"status":"Importedmesh' || bad "STL import status"
near "$(vol 13 2)" "$W3" "$TOL" || bad "OBJ import volume: $(vol 13 2) vs $W3 ($(bv 13))"
near "$(vol 13 1)" "$W3" "$TOL" || bad "first volume after the OBJ import: $(vol 13 1)"
[ "$(vol 16 1)" = "$(vol 13 1)" ] && [ "$(vol 16 2)" = "$(vol 13 2)" ] || bad "volumes after save and open: $(bv 16)"
# saved file
grep -q '"version": 29' "$SAVED" && grep -q '"Mesh"' "$SAVED" || bad "saved mesh file"
# an open mesh is refused
grep '"id":17,' "$OUT" | grep -q 'not closed' || bad "open mesh: $(grep '"id":17,' "$OUT")"
# IFC
[ "$(head -1 "$IFC")" = "ISO-10303-21;" ] || bad "ifc first line"
[ "$(tail -1 "$IFC")" = "END-ISO-10303-21;" ] || bad "ifc last line"
grep -q "FILE_SCHEMA(('IFC4'));" "$IFC" || bad "ifc schema"
for kv in IFCBUILDINGSTOREY:2 IFCWALL:4 IFCDOOR:1 IFCWINDOW:1 IFCOPENINGELEMENT:2 IFCRELVOIDSELEMENT:2 IFCRELFILLSELEMENT:2 IFCSPACE:1 IFCSLAB:1 IFCROOF:1 IFCSTAIR:1; do
  [ "$(grep -c "=${kv%%:*}(" "$IFC")" -eq "${kv##*:}" ] || bad "ifc ${kv%%:*}: $(grep -c "=${kv%%:*}(" "$IFC") instead of ${kv##*:}"
done
grep -q "IFCLABEL('EI60')" "$IFC" || bad "ifc property"
flat 20 | grep -qF '"status":"Exportedmcp_phase7c.ifc"' || bad "ifc status"

[ "$fail" -eq 0 ] && echo "MCP PHASE7C PASS" || { echo "outputs in $OUT"; exit 1; }
