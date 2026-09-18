from PIL import Image
import base64
import io

im = Image.open('assets/icon.png')
bbox = im.getbbox()
crop = im.crop(bbox)

# Clean center on 512x512 canvas
canvas_size = 512
padding = 40
inner_size = canvas_size - 2 * padding
w, h = crop.size
scale = min(inner_size / w, inner_size / h)
new_w = int(w * scale)
new_h = int(h * scale)
scaled = crop.resize((new_w, new_h), Image.Resampling.LANCZOS)

clean_icon = Image.new('RGBA', (canvas_size, canvas_size), (0, 0, 0, 0))
ox = (canvas_size - new_w) // 2
oy = (canvas_size - new_h) // 2
clean_icon.paste(scaled, (ox, oy))

# Save to all target paths
targets = [
    'icon.png',
    'assets/icon.png',
    'editors/vscode/icon.png',
    'editors/vscode/icons/icon.png'
]
for t in targets:
    clean_icon.save(t, 'PNG')
    print('Saved PNG:', t)

# Generate SVG
buf = io.BytesIO()
clean_icon.save(buf, format='PNG')
b64 = base64.b64encode(buf.getvalue()).decode('ascii')
svg = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="100%" height="100%">
  <image href="data:image/png;base64,{b64}" width="512" height="512" />
</svg>'''

svg_targets = [
    'icon.svg',
    'assets/icon.svg',
    'editors/vscode/icon.svg',
    'editors/vscode/icons/icon.svg'
]
for st in svg_targets:
    with open(st, 'w', encoding='utf-8') as f:
        f.write(svg)
    print('Saved SVG:', st)

print("Icon assets refreshed successfully!")
