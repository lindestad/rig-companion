from PIL import Image, ImageDraw
from pathlib import Path
p=Path('assets')
n=1024
im=Image.new('RGBA',(n,n),(0,0,0,0)); d=ImageDraw.Draw(im)
d.rounded_rectangle((24,24,1000,1000),radius=220,fill='#111113')
d.rounded_rectangle((185,300,839,697),radius=125,fill='#202024',outline='#eeeef0',width=42)
d.line((295,508,410,508,459,449,514,565,565,508,729,508),fill='#eeeef0',width=35)
d.ellipse((290,399,352,461),fill='#bca8ff');d.ellipse((672,399,734,461),fill='#bca8ff')
d.line((357,780,667,780),fill='#bca8ff',width=34)
im.resize((256,256),Image.Resampling.LANCZOS).save(p/'rig-companion.png')
im.save(p/'rig-companion.ico',sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
(p/'icon.rgba').write_bytes(im.resize((64,64),Image.Resampling.LANCZOS).tobytes())
