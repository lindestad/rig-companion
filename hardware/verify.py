"""Check KiCad's exported connectivity independently of symbol rendering."""
from pathlib import Path
import xml.etree.ElementTree as ET

root = Path(__file__).resolve().parent
nets = {n.attrib['name'].lstrip('/'): {(p.attrib['ref'], p.attrib['pin']) for p in n.findall('node')}
        for n in ET.parse(root/'rig-inputs.xml').findall('./nets/net')}
expected = {
    'JOY_X': {('U1','GPIO4'),('JY1','X')},
    'JOY_Y': {('U1','GPIO5'),('JY1','Y')},
    'JOY_PUSH': {('U1','GPIO21'),('JY1','SW_optional')},
    'V3V3': {('U1','3V3'),('JY1','VCC')},
    'GND': {('U1','GND'),('JY1','GND'),('ENC1','C'),('ENC1','SW2'),('ENC2','C'),('ENC2','SW2')} | {(f'SW{i}','2') for i in range(1,9)},
}
for i,(net,gpio) in enumerate(zip(['RESTORE','DASHBOARD','DESKTOP','UNDO','MOUSE_L','MOUSE_R','KEY_WIN','KEY_ESC'],range(6,14)),1):
    expected[net] = {('U1',f'GPIO{gpio}'),(f'SW{i}','1')}
for ref,prefix,pins in [('ENC1','HEIGHT',(15,16,17)),('ENC2','SCROLL',(18,1,2))]:
    for net,pin,gpio in zip(['A','B','PUSH'],['A','B','SW1'],pins):
        expected[f'{prefix}_{net}'] = {('U1',f'GPIO{gpio}'),(ref,pin)}
assert nets == expected, f'Connectivity mismatch: {nets}'
print('PASS: all 19 nets and every terminal match the intended hand-wiring circuit.')
