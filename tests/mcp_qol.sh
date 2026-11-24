#!/usr/bin/env bash
# End-to-end test of the QOL controls through `cad-app --mcp --background` only (no OS input): modal Grab / Rotate /
# Scale with axis lock and typed values, cancel, nudge, select all, the Settings window, control presets, rebinding,
# standard views, ortho and the focus mode.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_qol.out
rm -f "$OUT" target/mcp_qol_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
key() { call "$1" key "{\"key\":\"$2\"${3:+,$3}}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"qol","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. a line from (0,0) to (40,0); it stays selected
  cw 2 Line
  call 3 click '{"x":0,"y":0,"world":true}'
  call 4 click '{"x":40,"y":0,"world":true}'
  key 5 Escape
  call 6 state '{}'
  # 2. G X 10 Enter: moved 10 along X
  key 10 G
  key 11 X
  call 12 type_text '{"text":"10"}'
  call 13 state '{}'
  key 14 Enter
  call 15 state '{}'
  # 3. R 90 Enter about the centre (30,0); S 2 Enter
  key 20 R
  call 21 type_text '{"text":"90"}'
  key 22 Enter
  call 23 state '{}'
  key 24 S
  call 25 type_text '{"text":"2"}'
  key 26 Enter
  call 27 state '{}'
  # 4. G Y 5 then Escape: nothing moves; Ctrl+Z undoes the scale
  key 30 G
  key 31 Y
  call 32 type_text '{"text":"5"}'
  key 33 Escape
  call 34 state '{}'
  key 35 Z '"ctrl":true'
  call 36 state '{}'
  # 5. nudge: Right moves 1 mm, Shift+Right 10 mm
  key 40 ArrowRight
  key 41 ArrowRight '"shift":true'
  call 42 state '{}'
  # 6. a circle, then Ctrl+A selects both
  cw 50 Circle
  call 51 click '{"x":-50,"y":-50,"world":true}'
  call 52 click '{"x":-45,"y":-50,"world":true}'
  key 53 Escape
  key 54 A '"ctrl":true'
  call 55 state '{}'
  # 7. Settings window: Controls page, Blender preset; A selects all, X deletes, Ctrl+Z restores
  cw 60 "Open settings"
  cw 61 Controls
  cw 62 "Control preset"
  cw 63 Blender
  call 64 state '{}'
  call 65 screenshot '{}'
  cw 66 "Open settings"
  call 67 click '{"x":200,"y":200,"world":true}'
  key 68 A
  call 69 state '{}'
  key 70 X
  call 71 state '{}'
  key 72 Z '"ctrl":true'
  call 73 state '{}'
  # 8. rebind Fit all to H, then back to the Navigation page and the ProjectCAD preset
  cw 80 "Open settings"
  cw 81 "Fit all: key 0"
  call 82 state '{}'
  key 83 H
  call 84 state '{}'
  cw 85 "Open settings"
  # 9. 3D: Backtick toggles the view, 1 is the front view, 5 orthographic, Ctrl+Space hides the panels
  key 90 Backtick
  key 91 1
  key 92 5
  key 93 Space '"ctrl":true'
  call 94 state '{}'
  call 95 screenshot '{}'
  key 96 Space '"ctrl":true'
  call 97 state '{}'
} | CAD_MCP_SHOW="${CAD_MCP_SHOW:-}" target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_qol.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# the line's end points of state $1 as "ax ay bx by"
line() { flat "$1" | grep -oE '"Line":\{"a":\[[^]]*\],"b":\[[^]]*\]' | head -1 | grep -oE -- '-?[0-9][0-9.e-]*' | tr '\n' ' '; }
# line of state $1 within 1e-6 of ax ay bx by
isline() {
  set -- "$1" "$2" "$3" "$4" "$5" $(line "$1")
  near "$6" "$2" 1e-6 && near "$7" "$3" 1e-6 && near "$8" "$4" 1e-6 && near "$9" "$5" 1e-6
}
shot() { grep "\"id\":$1," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > "$2" 2>/dev/null; }
shot 65 target/mcp_qol_settings.png
shot 95 target/mcp_qol_3d.png
ents() { flat "$1" | grep -oE '"entities":[0-9]+' | head -1 | cut -d: -f2; }
sel() { flat "$1" | grep -oE '"selection":\[[^]]*\]' | head -1; }

isline 6 0 0 40 0 || bad "drawn line: $(line 6)"
flat 13 | grep -qF '"modal":{"axis":0,"mode":"Grab","plane":false,"typed":"10"}' || bad "modal while grabbing: $(flat 13 | grep -oE '"modal":[^}]*}')"
isline 15 10 0 50 0 || bad "after G X 10: $(line 15)"
flat 15 | grep -qF '"modal":null' || bad "modal still running after Enter"
isline 23 30 -20 30 20 || bad "after R 90: $(line 23)"
isline 27 30 -40 30 40 || bad "after S 2: $(line 27)"
isline 34 30 -40 30 40 || bad "Escape moved the line: $(line 34)"
flat 34 | grep -qF '"status":"Grabcancelled"' || bad "cancel status: $(flat 34 | grep -oE '"status":"[^"]*"')"
isline 36 30 -20 30 20 || bad "after undoing the scale: $(line 36)"
isline 42 41 -20 41 20 || bad "after nudging 1 + 10 mm: $(line 42)"
[ "$(sel 55)" = '"selection":[1,2]' ] || bad "Ctrl+A selection: $(sel 55)"
flat 64 | grep -qF '"preset":"Blender"' || bad "preset after the combo: $(flat 64 | grep -oE '"preset":"[^"]*"')"
flat 64 | grep -qF '"settings_open":true' || bad "settings window not open"
flat 64 | grep -qF '"orbit":"Middle"' || bad "Blender navigation: $(flat 64 | grep -oE '"nav":\{[^}]*\}')"
[ "$(sel 69)" = '"selection":[1,2]' ] || bad "Blender A: $(sel 69)"
[ "$(ents 71)" = 0 ] || bad "Blender X left $(ents 71) entities"
[ "$(ents 73)" = 2 ] || bad "undo after X: $(ents 73) entities"
flat 82 | grep -qF '"rebind":["Fit",0]' || bad "rebind not waiting: $(flat 82 | grep -oE '"rebind":[^,]*,?[^]]*\]?')"
flat 84 | grep -qF '"overrides":{"Fit":["H"]}' || bad "override after H: $(flat 84 | grep -oE '"overrides":\{[^}]*\}')"
flat 84 | grep -qF '"rebind":null' || bad "rebind still waiting"
flat 94 | grep -qF '"view":"ThreeD"' || bad "Backtick did not switch to 3D"
flat 94 | grep -qF '"ortho":true' || bad "5 did not switch to ortho"
flat 94 | grep -qF '"focus_mode":true' || bad "Ctrl+Space did not hide the panels"
flat 94 | grep -qE '"pitch":-?0(\.0+)?(e-?[0-9]+)?,' || bad "front view pitch: $(flat 94 | grep -oE '"camera3d":\{[^}]*\}')"
flat 94 | grep -qE '"yaw":-89\.9[0-9]*|"yaw":-90(\.0+)?[,}]' || bad "front view yaw: $(flat 94 | grep -oE '"camera3d":\{[^}]*\}')"
flat 97 | grep -qF '"focus_mode":false' || bad "Ctrl+Space did not show the panels again"
[ -s target/mcp_qol_settings.png ] || bad "no settings screenshot"
grep -q '"isError":true' "$OUT" && bad "a tool call failed: $(grep '"isError":true' "$OUT" | head -1)"

[ "$fail" -eq 0 ] && echo "MCP QOL PASS" || { echo "see $OUT"; exit 1; }
