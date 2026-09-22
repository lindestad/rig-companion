"""Rebuild reviewed estimates from the checked-in official identity snapshot.

Speeds are deliberately approximate, not measured records. See docs/wind-car-speeds.md.
No credentials, iRacing installation or network access needed.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
identity = json.loads((ROOT / 'data/iracing-car-identities.json').read_text(encoding='utf-8'))

# Ordered, specific variants before class-wide estimates. Values are km/h.
RULES = [
    (['formula vee'], 165), (['ray ff1600'], 210), (['mx-5'], 200),
    (['gr86'], 225), (['clio'], 220), (['m2 csr'], 250),
    (['m2 racing'], 270), (['caterham academy'], 195), (['caterham 420r'], 220),
    (['spec racer'], 240), (['skip barber'], 220), (['usf 2000'], 240),
    (['indy pro', 'pro mazda'], 270), (['dallara f3'], 275), (['fia f4'], 240),
    (['formula renault 2.0'], 250), (['formula renault 3.5'], 310),
    (['super formula lights'], 280), (['super formula sf23'], 330),
    (['dallara il-15'], 310), (['dallara ir18', 'dallara ir-05', 'dallara dw12'], 380),
    (['dallara ir-01'], 360), (['williams fw31', 'mercedes-amg w1', 'mclaren mp4-30'], 350),
    (['lotus 49'], 290), (['lotus 79'], 310),
    (['porsche 911 cup', 'gt3 cup', 'ferrari 296 challenge'], 290),
    (['gt3', 'mclaren mp4-12c', 'ruf rt 12r track'], 285), (['gt4'], 265),
    (['gt1'], 310), (['gte', '911 rsr', 'ford gt gt2'], 300),
    (['gtp', 'm hybrid', '499p', 'valkyrie'], 335),
    (['porsche 919', 'audi r18'], 340), (['dallara p217'], 325),
    (['hpd arx', 'daytona prototype', 'riley mk'], 310), (['ligier'], 290),
    (['radical sr10'], 290), (['radical sr8'], 280), (['mission r'], 300),
    (['tcr', 'kia optima'], 265), (['audi 90 gto'], 300), (['cadillac'], 275),
    (['fr500s'], 240), (['solstice'], 200), (['jetta'], 210),
    (['ruf rt 12r c-spec'], 285), (['ruf rt 12r'], 340),
    (['supercar', 'stock car pro series'], 300),
    (['euro nascar'], 250), (['next gen'], 315), (['1987'], 345),
    (['gen 4 grand national', 'xfinity', 'nationwide'], 310),
    (['nascar truck'], 300), (['gen 4', 'nascar cup'], 330), (['arca'], 300),
    (['whelen'], 250), (['modified - sk'], 220),
    (['dirt micro'], 160), (['dirt midget'], 180), (['dirt mini'], 155),
    (['dirt sprint car - 305'], 190), (['dirt sprint car - 360'], 210),
    (['dirt sprint car - 410'], 230), (['dirt sprint car non-winged - 360'], 200),
    (['dirt sprint car non-winged - 410'], 220),
    (['limited dirt late'], 180), (['pro dirt late'], 200), (['super dirt late'], 220),
    (['dirt big block'], 210), (['dirt 358'], 195), (['dirt ump'], 195),
    (['dirt street'], 175), (['dirt legends'], 180),
    (['mini stock'], 175), (['legends'], 195), (['street stock'], 210),
    (['super late'], 250), (['late model stock'], 235), (['silver crown'], 280),
    (['sprint car'], 260), (['cross car'], 160), (['pro 2 lite'], 150),
    (['pro 2 truck'], 180), (['pro 4 truck'], 190),
    (['beetle lite'], 180), (['beetle', 'fiesta', 'wrx'], 210),
]

cars = identity['cars'] + [
    {'name': name, 'path': ''} for name in [
        'BMW M2 Racing (G87)', 'EURO NASCAR RC01', 'Formula Vee - Cutlass',
        'Formula Vee - Conqueror', 'Formula Vee - Classic',
        'Aston Martin Valkyrie AMR-LMH', 'Caterham Academy', 'Caterham 420R',
        'Gen 4 Grand National', 'Supercars Chevrolet Camaro Gen 3',
        'NASCAR Truck RAM', "Dirt Legends Ford '34 Coupe",
        'BMW M4 GT3 EVO', 'BMW M Hybrid V8 Evo',
    ]
]
entries = {}
for car in cars:
    name = car['name']
    match = next(((words, speed) for words, speed in RULES if any(w in name.lower() for w in words)), None)
    if match is None:
        raise ValueError(f'Car needs a reviewed estimate: {name}')
    key = car['path'] or name
    if key in entries:  # Official table contains duplicate legacy/current listings.
        entries[key]['aliases'].append(name)
        continue
    entries[key] = {'name': name, 'aliases': [name, car['path']] if car['path'] else [name],
                    'top_speed_kmh': match[1], 'basis': 'Approximate car/class estimate'}
    if 'M2 Racing' in name:
        entries[key]['basis'] = 'iRacing 2026 S3 published 270 km/h; ruleset-dependent'
    if name == 'Radical SR10':
        entries[key]['basis'] = 'Radical published 180 mph, rounded to 290 km/h'

# Correct/extend known file-name and SimHub display-name variants without numeric-ID guesses.
for entry in entries.values():
    if entry['name'] == 'BMW M2 CSR': entry['aliases'] += ['bmwm2csr', 'BMW M2 CS Racing']
    if entry['name'] == 'FIA F4': entry['aliases'] += ['Formula iR-04', 'iRacing Formula iR-04']
    if entry['name'] == 'Global Mazda MX-5 Cup': entry['aliases'] += ['Mazda MX-5 Cup', 'Mazda MX-5 Cup 2016']
    if entry['name'] == 'Ray FF1600': entry['aliases'] += ['Ray Formula 1600']
    if entry['name'] == 'Renault Clio': entry['aliases'] += ['Renault Clio R.S. V']

data = {'reviewed': '2026-09-22', 'sources': [identity['source'],
    'https://support.iracing.com/support/solutions/articles/31000179016-2026-season-3-initial-release-notes-2026-06-09-01-',
    'https://support.iracing.com/support/solutions/articles/31000179517-2026-season-4-initial-release-notes-2026-09-09-01-',
    'https://radicalmotorsport.com/news/50th-production-sr10'],
    'cars': list(entries.values())}
(ROOT / 'data/wind-car-speeds.json').write_text(json.dumps(data, indent=2, ensure_ascii=False)+'\n', encoding='utf-8')
print(f'{len(entries)} car entries, every official identity assigned an estimate')
