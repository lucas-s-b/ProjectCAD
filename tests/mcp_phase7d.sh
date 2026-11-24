#!/usr/bin/env bash
# End-to-end test of docked panels, themes and their settings file, command search, the rotate gizmo and gizmo drags of jointed components, through `cad-app --mcp` only. No OS input.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase7d.out
SET=target/mcp_phase7d_settings.json
GEARS=target/mcp_phase7d_gears.cadproj
PIN=target/mcp_phase7d_pin.cadproj
rm -f "$OUT" "$SET" "$SET.1" "$GEARS" "$PIN" target/mcp_phase7d.err

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

target/debug/cad-bench.exe assembly "$GEARS" 3 100 || { echo "cannot write $GEARS"; exit 1; }
target/debug/cad-bench.exe pin "$PIN" || { echo "cannot write $PIN"; exit 1; }
# init <client name>: the initialize request and notification of one app launch
init() { printf '%s\n' "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"$1\",\"version\":\"0\"}}}" '{"jsonrpc":"2.0","method":"notifications/initialized"}'; }

{ init phase7d
  # 1. theme, accent and layout from the View menu (each entry closes the menu)
  call 3 state '{}'
  cw 4 View
  cw 5 "Light theme"
  cw 6 View
  cw 7 "Accent: Orange"
  cw 8 View
  cw 9 "Model: Left"
  cw 10 View
  cw 11 "Assembly: Bottom"
  cw 12 Model
  call 13 widgets '{}'
  call 14 state '{}'
  # 2. command search: Ctrl+K, a fuzzy query, Enter clicks the best match ("Add part" adds the gear)
  cw 15 Gear
  call 16 key '{"key":"K","ctrl":true}'
  call 17 type_text '{"text":"addprt"}'
  call 18 state '{}'
  call 19 key '{"key":"Enter"}'
  call 20 state '{}'
  call 21 key '{"key":"K","ctrl":true}'
  call 22 type_text '{"text":"zzzz"}'
  call 23 state '{}'
  call 24 key '{"key":"Escape"}'
  call 25 state '{}'
} | CAD_SETTINGS="$SET" target/debug/cad-app.exe --mcp --background >> "$OUT" 2>> target/mcp_phase7d.err
# the file as the first launch left it (the second launch resets the layout)
cp "$SET" "$SET.1"
{ init phase7d
  # 3. the settings come back; Reset layout restores the default screen
  call 101 state '{}'
  cw 102 View
  cw 103 "Reset layout"
  call 104 state '{}'
} | CAD_SETTINGS="$SET" target/debug/cad-app.exe --mcp --background >> "$OUT" 2>> target/mcp_phase7d.err
{ init phase7d
  # 4. without CAD_SETTINGS an MCP run starts from the defaults
  call 201 state '{}'
  # 5. rotate gizmo: gear 2 turns 90 deg about Z, gear 3 90 deg about X (Shift snaps to 15 deg)
  call 202 open_project "{\"path\":\"$GEARS\"}"
  cw 203 3D
  call 204 key '{"key":"F"}'
  cw 205 Assembly
  cw 206 "Gear #2"
  call 207 state '{}'
  # gizmo_rings of state 207 is [[[612,304],[533,364]],[[646,350],[640,483]],[[675,387],[550,353]]] (45 and 135 degree points of the X, Y, Z rings): drag the Z ring from its 45 to its 135 degree point with Shift
  call 208 drag '{"x1":675,"y1":387,"x2":550,"y2":353,"button":"primary","shift":true}'
  call 209 state '{}'
  call 210 key '{"key":"Z","ctrl":true}'
  call 211 state '{}'
  cw 212 "Gear #3"
  call 213 state '{}'
  # gizmo_rings of state 213 is [[[803,348],[745,422]],[[864,404],[837,547]],[[871,439],[720,398]]]: the X ring from 45 to 135 degrees with Shift
  call 214 drag '{"x1":803,"y1":348,"x2":745,"y2":422,"button":"primary","shift":true}'
  call 215 state '{}'
  # 6. the pin on its revolute joint: the gizmo ring turns it 90 deg
  call 216 open_project "{\"path\":\"$PIN\"}"
  call 217 key '{"key":"F"}'
  cw 218 c2
  call 219 state '{}'
  # gizmo_joint of state 219 is {"kind":"Revolute","points":[[599,387],[572,342],[495,401]]} (centre, 45 deg, 135 deg): drag 45 to 135 with Shift
  call 220 drag '{"x1":572,"y1":342,"x2":495,"y2":401,"button":"primary","shift":true}'
  call 221 state '{}'
  call 222 key '{"key":"Z","ctrl":true}'
  call 223 state '{}'
  # 7. the same joint as a slider: the gizmo arrow pushes the pin along Z
  cw 224 "Joint 1"
  cw 225 "Joint type"
  cw 226 "Type: Slider"
  st 227 "Joint value" "0 mm"
  cw 228 c2
  call 229 state '{}'
  # gizmo_joint of state 229 is {"kind":"Slider","points":[[599,387],[602,215]]} (origin, tip): drag from the middle of the arrow (601,301) to half an arrow past the tip (604,129)
  call 230 drag '{"x1":601,"y1":301,"x2":604,"y2":129,"button":"primary"}'
  call 231 state '{}'
} | target/debug/cad-app.exe --mcp --background >> "$OUT" 2>> target/mcp_phase7d.err

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
# $1 < $2
lt() { awk -v a="$1" -v b="$2" 'BEGIN {exit !(a != "" && a < b)}'; }
# the bodies array of state $1
bv() { flat "$1" | grep -oE '"bodies":\[[^]]*\]'; }
# the settings object of state $1
sv() { flat "$1" | grep -oE '"settings":\{"accent":\[[^]]*\],"controls":.*"theme":"[A-Za-z]+"'; }
# the view rectangle of state $1
vr() { flat "$1" | grep -oE '"view2d":\{"center":\[[^]]*\],"rect":\[[^]]*\]' | grep -oE '"rect":.*'; }
# the widget named $2 (spaces removed) in the widgets reply $1
wg() { flat "$1" | grep -oE "\{[^{}]*\"name\":\"$2\"[^{}]*\}" | head -1; }
# component $2 of state $1: angles, at, body, grounded, id
comp() { flat "$1" | grep -oE "\"angles\":\[[^]]*\],\"at\":\[[^]]*\],\"body\":[0-9]+,\"grounded\":(true|false),\"id\":$2[,}]"; }
# the joints rows of state $1
jv() { flat "$1" | grep -oE '"joints":\[[^]]*\]'; }

[ "$(grep -c . "$OUT")" -ge 58 ] || bad "expected at least 58 responses, got $(grep -c . "$OUT")"

# 3: the defaults
sv 3 | grep -qF '"theme":"Dark"' && sv 3 | grep -qF '"docks":["Right","Right","Right"]' || bad "default settings: $(sv 3)"
VR=$(vr 3)
[ -n "$VR" ] || bad "view rectangle of state 3"
# 14: theme, accent and layout
sv 14 | grep -qF '"accent":[200,100,20]' && sv 14 | grep -qF '"docks":["Right","Left","Bottom"]' && sv 14 | grep -qF '"theme":"Light"' || bad "settings after the View menu: $(sv 14)"
lt "$(wg 13 Addpart | num x)" 640 || bad "Add part is not in the left panel: $(wg 13 Addpart)"
ge "$(wg 13 Componentbody | num y)" 450 || bad "Component body is not in the bottom slot: $(wg 13 Componentbody)"
# 18..25: command search
flat 18 | grep -qF '"palette":{"matches":["Addpart"' && flat 18 | grep -qF '"query":"addprt"' || bad "palette matches: $(flat 18 | grep -oE '"palette":.{0,200}')"
flat 20 | grep -qF '"palette":null' || bad "palette still open after Enter"
bv 20 | grep -q '"volume"' || bad "the command search did not add the gear: $(bv 20)"
flat 23 | grep -qF '"matches":[]' || bad "palette matches for zzzz: $(flat 23 | grep -oE '"palette":.{0,200}')"
flat 25 | grep -qF '"palette":null' || bad "palette still open after Escape"
# the settings file
grep -q '"theme": "Light"' "$SET.1" && grep -q '"Left"' "$SET.1" || bad "settings file: $(cat "$SET.1" 2>&1)"
# 101, 104: reload and reset
[ "$(sv 101)" = "$(sv 14)" ] || bad "settings after reload: $(sv 101) vs $(sv 14)"
sv 104 | grep -qF '"docks":["Right","Right","Right"]' && sv 104 | grep -qF '"theme":"Light"' || bad "settings after Reset layout: $(sv 104)"
[ "$(vr 104)" = "$VR" ] || bad "the default screen did not come back: $(vr 104) vs $VR"
# 201: no settings file without CAD_SETTINGS
sv 201 | grep -qF '"theme":"Dark"' && sv 201 | grep -qF '"accent":[0,92,128]' || bad "settings without CAD_SETTINGS: $(sv 201)"
# 5. the rotate gizmo
flat 207 | grep -q '"gizmo":\[\[' && flat 207 | grep -q '"gizmo_rings":\[\[\[' || bad "gizmo and rings of state 207"
comp 209 2 | grep -qF '"angles":["0deg","0deg","90deg"],"at":["100mm","0mm","0mm"]' || bad "gear 2 after the Z turn: $(comp 209 2)"
comp 211 2 | grep -qF '"angles":["0","0","0"]' || bad "gear 2 after undo: $(comp 211 2)"
comp 215 3 | grep -qF '"angles":["90deg","0deg","0deg"],"at":["200mm","5mm","5mm"]' || bad "gear 3 after the X turn: $(comp 215 3)"
# 6. the joint ring
flat 219 | grep -q '"gizmo":null' && flat 219 | grep -qF '"gizmo_joint":{"kind":"Revolute","points":[[' || bad "gizmo of state 219"
jv 221 | grep -qF '"value":"90deg"' || bad "joint after the ring drag: $(jv 221)"
jv 223 | grep -qF '"value":"0deg"' || bad "joint after undo: $(jv 223)"
# 7. the joint as a slider
flat 229 | grep -qF '"gizmo_joint":{"kind":"Slider"' || bad "slider handle in state 229"
V=$(jv 231 | grep -oE '"value":"[-0-9.]+mm"' | grep -oE '[-0-9.]+' | head -1)
ge "$V" 1 || bad "slider value after the arrow drag: '$V' ($(jv 231))"

[ "$fail" -eq 0 ] && echo "MCP PHASE7D PASS" || { echo "see $OUT"; exit 1; }
