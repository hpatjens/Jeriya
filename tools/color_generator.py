from colour import Color
from PIL import Image, ImageDraw

def hex_to_rgba(hex_color):
    # Remove the '#' character if present
    hex_color = hex_color.lstrip('#')

    # Convert hex to RGB
    r = int(hex_color[0:2], 16) / 255.0
    g = int(hex_color[2:4], 16) / 255.0
    b = int(hex_color[4:6], 16) / 255.0

    # Return the RGBA tuple
    return r, g, b, 1.0  # Assuming full opacity

def write_image(colors):
    STEP_SIZE = 10

    # Create Image
    width = 50
    height = (len(colors) - 1) * STEPS * STEP_SIZE
    image = Image.new("RGB", (width, height), (255, 255, 255))
    draw = ImageDraw.Draw(image)

    # Draw Gradient
    y = 0
    for sub_array in colors:
        for color in sub_array:
            r, g, b, a = hex_to_rgba(color.hex_l)
            r, g, b, a = int(r * 255), int(g * 255), int(b * 255), int(a * 255)

            shape = [(0, y), (width, y + STEP_SIZE - 2)] 

            draw.rectangle(shape, fill=(r, g, b))  # Draw a red rectangle
            y += STEP_SIZE

    # Save Image
    image.save("gradient.png")
    image.close()



# Generate with https://mycolor.space/
initial_colors = [
    Color("#E6F800"),
    Color("#71E05A"),
    Color("#00BE84"),
    Color("#009794"),
    Color("#006F83"),
    Color("#2F4858"),
];

STEPS = 10
colors = []
for i in range(0, len(initial_colors) - 1):
    result = list(initial_colors[i].range_to(initial_colors[i + 1], STEPS))
    colors.append(result)

i = 0
for sub_array in colors:
    for color in sub_array:
        print(f"{color.hex_l}")
        i += 1

i = 0
for sub_array in colors:
    for color in sub_array:
        r, g, b, a = hex_to_rgba(color.hex_l)
        print(f"({r:.4f}, {g:.4f}, {b:.4f})")
        i += 1

write_image(colors)