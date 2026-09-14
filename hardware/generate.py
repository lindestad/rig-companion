"""Generate the editable wiring schematic and harness list. Python stdlib only."""
from pathlib import Path
import csv
import json
import uuid

ROOT = Path(__file__).resolve().parent
uid = lambda: str(uuid.uuid4())
q = lambda s: json.dumps(str(s))
root_id = uid()
parts, libraries, wires = [], [], []

def text(s, x, y, size=1.5):
    parts.append(f'(text {q(s)} (at {x} {y} 0) (effects (font (size {size} {size})) (justify left)) (uuid {uid()}))')

def block(name, ref, title, pins, x, y, width=43, step=6):
    """Logical terminal symbol; names are device markings, not header ordinals."""
    x,y,step = [round(round(v/1.27)*1.27,4) for v in (x,y,step)]
    height = (len(pins)-1)*step+8
    inner = [f'(rectangle (start 0 4) (end {width} {-height+4}) (stroke (width .254) (type default)) (fill (type background)))']
    for i, (pin, net) in enumerate(pins):
        inner.append(f'(pin passive line (at -5.08 {-i*step} 0) (length 5.08) (name {q(pin)} (effects (font (size 1.27 1.27)))) (number {q(pin)} (effects (font (size 1 1)))))')
    libraries.append(f'(symbol "Rig:{name}" (pin_names (offset 1)) (pin_numbers hide) (in_bom yes) (on_board yes) (property "Reference" "{ref}" (at 0 8 0) (effects (font (size 1.5 1.5)))) (property "Value" {q(title)} (at 0 6 0) (effects (font (size 1.5 1.5)))) (symbol "{name}_0_1" {inner[0]}) (symbol "{name}_1_1" {" ".join(inner[1:])}))')
    symbol_id = uid()
    parts.append(f'(symbol (lib_id "Rig:{name}") (at {x} {y} 0) (unit 1) (in_bom yes) (on_board yes) (dnp no) (uuid {symbol_id}) (property "Reference" "{ref}" (at {x} {y-10} 0) (effects (font (size 1.5 1.5)) (justify left))) (property "Value" {q(title)} (at {x} {y-6} 0) (effects (font (size 1.5 1.5)) (justify left))) {" ".join(f"(pin {q(p)} (uuid {uid()}))" for p,n in pins)} (instances (project "rig-inputs" (path "/{root_id}" (reference "{ref}") (unit 1)))))')
    for i, (pin, net) in enumerate(pins):
        yy = y+i*step
        parts.append(f'(wire (pts (xy {x-25.4} {yy}) (xy {x-5.08} {yy})) (stroke (width 0) (type default)) (uuid {uid()}))')
        parts.append(f'(label {q(net)} (at {x-25.4} {yy} 0) (effects (font (size 1.27 1.27)) (justify left bottom)) (uuid {uid()}))')

keys = [('Restore height',6,'RESTORE'), ('Dashboard',7,'DASHBOARD'), ('Desktop request',8,'DESKTOP'), ('Undo',9,'UNDO'), ('Left mouse',10,'MOUSE_L'), ('Right mouse',11,'MOUSE_R'), ('Windows key',12,'KEY_WIN'), ('Escape',13,'KEY_ESC')]
inputs = [('4','JOY_X'),('5','JOY_Y')] + [(str(g),n) for _,g,n in keys] + [('15','HEIGHT_A'),('16','HEIGHT_B'),('17','HEIGHT_PUSH'),('18','SCROLL_A'),('1','SCROLL_B'),('2','SCROLL_PUSH'),('21','JOY_PUSH')]
board = [('3V3','V3V3'),('GND','GND')] + [('GPIO'+p,n) for p,n in inputs]
block('Devkit','U1','ESP32-S3 DevKitC-1 / logical pins',board,52,44,60,7)
block('Joystick','JY1','Analog stick / breakout REQUIRED', [('VCC','V3V3'),('GND','GND'),('X','JOY_X'),('Y','JOY_Y'),('SW_optional','JOY_PUSH')],175,44,62)
block('HeightEncoder','ENC1','Height encoder + push', [('A','HEIGHT_A'),('B','HEIGHT_B'),('C','GND'),('SW1','HEIGHT_PUSH'),('SW2','GND')],175,100,62)
block('ScrollEncoder','ENC2','Scroll encoder + push', [('A','SCROLL_A'),('B','SCROLL_B'),('C','GND'),('SW1','SCROLL_PUSH'),('SW2','GND')],175,155,62)

# Actual normally-open switch graphic, two interchangeable solder contacts.
libraries.append('(symbol "Rig:Key" (pin_names hide) (pin_numbers hide) (in_bom yes) (on_board yes) (property "Reference" "SW" (at 0 5 0) (effects (font (size 1.27 1.27)))) (property "Value" "MX" (at 0 -5 0) (effects (font (size 1.27 1.27)))) (symbol "Key_0_1" (polyline (pts (xy -3 0) (xy 3 3)) (stroke (width .254) (type default)) (fill (type none))) (symbol "Key_1_1" (pin passive line (at -8 0 0) (length 5) (name "1" (effects (font (size 1 1)))) (number "1" (effects (font (size 1 1))))) (pin passive line (at 8 0 180) (length 5) (name "2" (effects (font (size 1 1)))) (number "2" (effects (font (size 1 1)))))))')
libraries[-1] = libraries[-1].replace('(symbol "Key_1_1"', ') (symbol "Key_1_1"')
libraries[-1] = libraries[-1].replace('(at -8 0 0)', '(at -7.62 0 0)').replace('(at 8 0 180)', '(at 7.62 0 180)').replace('(length 5)', '(length 4.62)')
for i,(title,gpio,net) in enumerate(keys):
    x,y,ref=335,48+i*19,f'SW{i+1}'
    x,y=[round(round(v/1.27)*1.27,4) for v in (x,y)]
    parts.append(f'(symbol (lib_id "Rig:Key") (at {x} {y} 0) (unit 1) (in_bom yes) (on_board yes) (dnp no) (uuid {uid()}) (property "Reference" "{ref}" (at {x-8} {y-6} 0) (effects (font (size 1.3 1.3)) (justify left))) (property "Value" {q(title)} (at {x+2} {y-6} 0) (effects (font (size 1.3 1.3)) (justify left))) (pin "1" (uuid {uid()})) (pin "2" (uuid {uid()})) (instances (project "rig-inputs" (path "/{root_id}" (reference "{ref}") (unit 1)))))')
    for a,b,n in [(x-38.1,x-7.62,net),(x+7.62,x+38.1,'GND')]:
        parts.append(f'(wire (pts (xy {a} {y}) (xy {b} {y})) (stroke (width 0) (type default)) (uuid {uid()}))')
        label_x = b if n == 'GND' else a
        parts.append(f'(label {q(n)} (at {label_x} {y} 0) (effects (font (size 1.27 1.27)) (justify left bottom)) (uuid {uid()}))')
    wires.append([net,f'U1 GPIO{gpio}',f'{ref} contact 1',title])
    wires.append(['GND','U1 GND / ground bus',f'{ref} contact 2','Black; daisy-chain within key bank'])
for dest,signal,pin in [('JY1 X','JOY_X',4),('JY1 Y','JOY_Y',5),('JY1 SW (optional)','JOY_PUSH',21),('ENC1 A','HEIGHT_A',15),('ENC1 B','HEIGHT_B',16),('ENC1 SW1','HEIGHT_PUSH',17),('ENC2 A','SCROLL_A',18),('ENC2 B','SCROLL_B',1),('ENC2 SW1','SCROLL_PUSH',2)]:
    wires.append([signal,f'U1 GPIO{pin}',dest,'Label both wire ends'])
for dest in ['JY1 GND','ENC1 C','ENC1 SW2','ENC2 C','ENC2 SW2']:
    wires.append(['GND','U1 GND / ground bus',dest,'Black'])
wires.append(['V3V3','U1 3V3','JY1 VCC','Red; ONLY after joystick identification'])

text('RIG INPUTS  /  HAND-WIRED PROTOTYPE A',20,17,3)
text('8 MX keys  +  2 push encoders  +  analog mouse stick  /  one native USB cable',20,25,1.8)
notes=[
    'CONNECTIONS: identical net labels are electrically connected. Pin names are functional markings, NOT physical header positions.',
    'USB: PC -> board native USB/OTG socket. GPIO19 and GPIO20 reserved. UART-only USB socket cannot carry our HID reports.',
    'KEYS: two electrical contacts per MX switch; either orientation. Enable INPUT_PULLUP on all keys, encoder A/B and push inputs.',
    'ENCODERS: passive mechanical A/B/C + isolated push switch. Join C and SW2 locally to GND; 4 wires per encoder module.',
    'JOYSTICK: 3.3 V analog assumption ONLY. Flex pin count/order/pitch and click are UNVERIFIED. No physical FPC pin assignment.',
    'Four-wire stick: leave GPIO21 unconnected, disable its action; SW5 is left click. Five-wire breakout: click may also use GPIO21.',
    'No matrix, diodes or external pull-ups in minimum build. Firmware debounce required. Optional 100 nF from each ADC input to GND.',
    'Use 2.54 mm solder headers or prewired keyed plugs. Keep GND with each cable. Insulate solder joints and add strain relief.',
    'REFERENCE BOARD ONLY: check exact clone schematic before wiring. No PCB footprint or manufacturing pinout is implied here.',
]
for i,n in enumerate(notes): text(n,20,211+i*5,1.4)
content=f'(kicad_sch (version 20250114) (generator "eeschema") (uuid {root_id}) (paper "A3") (title_block (title "Rig inputs - hand-wired prototype") (date "2026-09-14") (rev "A") (comment 1 "Logical wiring; joystick flex mapping pending")) (lib_symbols {" ".join(libraries)}) {" ".join(parts)})'
(ROOT/'rig-inputs.kicad_sch').write_text(content,encoding='utf-8')
(ROOT/'Rig.kicad_sym').write_text('(kicad_symbol_lib (version 20231120) (generator "kicad_symbol_editor") '+ ' '.join(libraries).replace('"Rig:', '"')+')', encoding='utf-8')
(ROOT/'sym-lib-table').write_text('(sym_lib_table (lib (name "Rig") (type "KiCad") (uri "${KIPRJMOD}/Rig.kicad_sym") (options "") (descr "Logical hand-wiring terminals")))\n',encoding='utf-8')
with (ROOT/'wire-list.csv').open('w',newline='',encoding='utf-8') as f:
    w=csv.writer(f);w.writerow(['Net','From','To','Assembly note']);w.writerows(wires)
assert len({p for p,n in inputs})==17
assert not ({int(p) for p,n in inputs}&{0,3,19,20,35,36,37,38,45,46,48})
print(f'Generated schematic and {len(wires)} wire connections')
