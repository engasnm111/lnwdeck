import os
from PIL import Image, ImageDraw, ImageFont

icons_dir = r"E:\lnwdeck\apps\desktop\src-tauri\icons"
os.makedirs(icons_dir, exist_ok=True)

def create_base_icon(size):
    # Dark blue/purple background with rounded circle
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    
    # Outer circle with accent color #6f7df6
    margin = int(size * 0.05)
    bg_color = (11, 16, 32, 255) # #0b1020
    border_color = (111, 125, 246, 255) # #6f7df6
    
    draw.ellipse([margin, margin, size - margin, size - margin], fill=bg_color, outline=border_color, width=max(1, int(size * 0.06)))
    
    # Draw stylized 'L' shape in center
    cx, cy = size // 2, size // 2
    w = max(2, int(size * 0.12))
    
    # Draw L shape accent
    draw.line([(int(size * 0.32), int(size * 0.28)), (int(size * 0.32), int(size * 0.68))], fill=border_color, width=w)
    draw.line([(int(size * 0.32), int(size * 0.68)), (int(size * 0.68), int(size * 0.68))], fill=border_color, width=w)
    
    # Accent dot
    dot_r = max(1, int(size * 0.08))
    draw.ellipse([int(size * 0.68) - dot_r, int(size * 0.35) - dot_r, int(size * 0.68) + dot_r, int(size * 0.35) + dot_r], fill=(53, 201, 139, 255))
    
    return img

sizes = [16, 24, 32, 48, 64, 128, 256]
images = {}

for s in sizes:
    img = create_base_icon(s)
    images[s] = img
    png_path = os.path.join(icons_dir, f"{s}x{s}.png")
    img.save(png_path, format="PNG")

# Special names expected by Tauri
images[256].save(os.path.join(icons_dir, "icon.png"), format="PNG")
images[256].save(os.path.join(icons_dir, "128x128@2x.png"), format="PNG")
images[128].save(os.path.join(icons_dir, "128x128.png"), format="PNG")
images[32].save(os.path.join(icons_dir, "32x32.png"), format="PNG")

# Generate multi-resolution ICO file
ico_path = os.path.join(icons_dir, "icon.ico")
images[256].save(ico_path, format="ICO", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])

print("Successfully generated icon assets in sizes 16 to 256 and icon.ico!")
