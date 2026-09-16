"""Refresh UI facts/help from the local sboys3 checkout; Python stdlib only."""
from html.parser import HTMLParser
from pathlib import Path
import json, re, sys

class Node:
    def __init__(self, tag='', attrs=(), parent=None):
        self.tag, self.attrs, self.parent, self.children, self.text = tag, dict(attrs), parent, [], ''
    def walk(self):
        yield self
        for c in self.children: yield from c.walk()
    def content(self): return self.text + ''.join(c.content() for c in self.children)

class Parser(HTMLParser):
    def __init__(self):
        super().__init__(); self.root=Node(); self.current=self.root
    def handle_starttag(self,tag,attrs):
        n=Node(tag,attrs,self.current); self.current.children.append(n)
        if tag not in ('input','br','hr','img','link','meta'): self.current=n
    def handle_endtag(self,tag):
        n=self.current
        while n.parent:
            if n.tag==tag: self.current=n.parent; return
            n=n.parent
    def handle_startendtag(self,tag,attrs):
        self.handle_starttag(tag,attrs); self.handle_endtag(tag)
    def handle_data(self,data): self.current.text += data

repo=Path(sys.argv[1]) if len(sys.argv)>1 else Path.home()/'dev/CustomHeadsetOpenVR'
base=repo/'CustomHeadsetGUI/src/app/pages/devices'
metadata={}
for name in ('headset-settings','pimax-slam-settings','general'):
    parser=Parser(); parser.feed((base/name/(name+'.component.html')).read_text(encoding='utf-8'))
    for field in parser.root.walk():
        if field.tag!='div' or 'field' not in field.attrs.get('class','').split(): continue
        nodes=list(field.walk())
        models={n.attrs.get('[(ngmodel)]','') for n in nodes}
        for model in sorted(models):
            if not re.fullmatch(r'settings\.[\w.]+',model): continue
            key=model.removeprefix('settings.').replace('.','/')
            if name!='general': key='headset/'+key
            entry=metadata.setdefault(key,{})
            tip=next((n for n in nodes if n.tag=='app-field-tip'),None)
            if tip and 'info' in tip.attrs: entry['help']=' '.join(tip.attrs['info'].split())
            note=next((n for n in nodes if n.tag=='app-field-note'),None)
            if note: entry['restart']=note.attrs.get('type','')
            number=next((n for n in nodes if n.attrs.get('[(ngmodel)]')==model and n.attrs.get('type')=='number'),None)
            if number: entry['step']=float(number.attrs.get('step',1))
            slider=next((n for n in nodes if n.tag=='mat-slider' and any(c.attrs.get('[(ngmodel)]')==model for c in n.walk())),None)
            if slider:
                entry['step']=float(slider.attrs.get('step',entry.get('step',1)))
                entry['min']=float(slider.attrs['min'])
                if 'max' in slider.attrs: entry['max']=float(slider.attrs['max'])
                else: entry['dynamic_fov']=model[-1].lower()
            select=next((n for n in nodes if n.tag=='mat-select' and n.attrs.get('[(ngmodel)]')==model),None)
            if select:
                options=[]
                for n in select.walk():
                    if n.tag!='mat-option': continue
                    v=n.attrs.get('value',n.attrs.get('[value]'))
                    if v is None or re.search(r'opt|p\.|i$',v): continue
                    options.append({'value':v,'label':' '.join(n.content().split())})
                if options: entry['options']=options
for name in ('ipd','hardwareIpd'):
    metadata['headset/'+name]['help']='Match the rendered IPD to the physical headset IPD for correct world scale. This is the distance between the virtual cameras in SteamVR applications.'
metadata['headset/enable']['help']='Enable the Custom Headset driver for this headset. Restart SteamVR to apply.'
metadata['headset/disableEye']['options']=[{'value':'0','label':'Both eyes'},{'value':'1','label':'Disable left'},{'value':'2','label':'Disable right'},{'value':'3','label':'Disable both'}]
metadata['headset/proximitySensorType']['options']=[{'value':str(i),'label':n} for i,n in enumerate(['None','Hardware','Stationary dimming','Always on','Always off'])]
# Dynamic FOV sliders use live driver resolution limits, with headset defaults as fallback.
out=Path(__file__).resolve().parents[1]/'assets/driver-controls.json'
out.write_text(json.dumps(metadata,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(f'{len(metadata)} control definitions written to {out}')
