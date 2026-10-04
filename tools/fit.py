#!/usr/bin/env python3
"""Overlays the player (humanoid::wanderer) on a reference picture, to tweak him until he matches.
Each tools/fits/<name>.json names a reference image and poses the body and a camera:
  image     "ref images/….png"
  crop      [left, top, right, bottom], fractions of the image to keep (default whole)
  pose      "relaxed" | "ready" | "guard" | "shout" | "crouch" | "swing 0.4"   (the game's poses)
  joints    {"ArmL": [x, y, z], …} radians added to the pose (same YXZ Euler as the game)
  lean, drop  pelvis tilt (rad) and lowering (m)
  camera    at: [x, y, z]  model point it looks at (metres, feet at y = 0, he faces -z)
            pin: [u, v]    where `at` lands, fraction of the cropped image (default [0.5, 0.5])
            yaw: deg, + moves the camera round toward his right (sword arm); pitch: deg, + from above
            roll: deg; distance: m; fov: vertical deg of the cropped image (small ≈ orthographic)
The mesh comes from screenshots/fit/<name>.bin, dumped by `tools/fit` (which runs this afterwards).
Usage: tools/fit.py [name …]  → screenshots/fit/<name>.png: reference | model | blend | outline"""
import glob, json, math, os, sys
import numpy as np
from PIL import Image, ImageFilter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from heads import load

HEIGHT = 800
METAL = ("Iron", "Steel", "Gold")


def camera(spec, width, height):
    at = np.array(spec.get("at", [0, 1.1, 0]), dtype=np.float32)
    yaw, pitch, roll = (math.radians(spec.get(key, 0.0)) for key in ("yaw", "pitch", "roll"))
    away = np.array([math.sin(yaw) * math.cos(pitch), math.sin(pitch), -math.cos(yaw) * math.cos(pitch)])
    eye = at + spec.get("distance", 6.0) * away
    forward = (at - eye) / np.linalg.norm(at - eye)
    right = np.cross(forward, [0, 1, 0]); right /= np.linalg.norm(right)
    up = np.cross(right, forward)
    right, up = right * math.cos(roll) + up * math.sin(roll), up * math.cos(roll) - right * math.sin(roll)
    focal = height / 2 / math.tan(math.radians(spec.get("fov", 20.0)) / 2)
    pin = spec.get("pin", [0.5, 0.5])
    return eye, np.stack([right, up, forward]).astype(np.float32), focal, (pin[0] * width, pin[1] * height)


def fragments(xy, z, shade, width, height):
    """Every covered pixel of every triangle: (pixel index, depth, colour)."""
    low = np.floor(xy.min(axis=1)).astype(int)
    span = (np.ceil(xy.max(axis=1)).astype(int) - low).max(axis=1) + 1
    area = ((xy[:, 1, 0] - xy[:, 0, 0]) * (xy[:, 2, 1] - xy[:, 0, 1])
            - (xy[:, 2, 0] - xy[:, 0, 0]) * (xy[:, 1, 1] - xy[:, 0, 1]))
    keep = (np.abs(area) > 1e-9) & (low[:, 0] < width) & (low[:, 1] < height) \
        & (low[:, 0] + span > 0) & (low[:, 1] + span > 0)
    out = []
    size = 1
    while keep.any():
        chosen = np.nonzero(keep & (span <= size))[0]
        keep[chosen] = False
        step = max(1, 3_000_000 // (size * size))
        offsets = np.stack(np.meshgrid(np.arange(size), np.arange(size)), -1).reshape(-1, 2)
        for start in range(0, len(chosen), step):
            tri = chosen[start:start + step]
            px = low[tri, None, :] + offsets[None]
            gx, gy = px[..., 0] + 0.5, px[..., 1] + 0.5
            xs, ys, a = xy[tri, :, 0], xy[tri, :, 1], area[tri, None]
            w0 = ((xs[:, 1, None] - gx) * (ys[:, 2, None] - gy) - (xs[:, 2, None] - gx) * (ys[:, 1, None] - gy)) / a
            w1 = ((xs[:, 2, None] - gx) * (ys[:, 0, None] - gy) - (xs[:, 0, None] - gx) * (ys[:, 2, None] - gy)) / a
            w2 = 1 - w0 - w1
            inside = (w0 >= 0) & (w1 >= 0) & (w2 >= 0) & (px[..., 0] >= 0) & (px[..., 0] < width) \
                & (px[..., 1] >= 0) & (px[..., 1] < height)
            which, slot = np.nonzero(inside)
            weights = np.stack([w0[which, slot], w1[which, slot], w2[which, slot]], -1)
            t = tri[which]
            out.append((px[which, slot, 1] * width + px[which, slot, 0],
                        (weights * z[t]).sum(-1),
                        (weights[..., None] * shade[t]).sum(1)))
        size *= 2
    return [np.concatenate(parts) for parts in zip(*out)] if out else None


def render(groups, spec, width, height):
    eye, turn, focal, (cx, cy) = camera(spec, width, height)
    light = np.array([-0.45, 0.55, -0.7]) @ turn
    light /= np.linalg.norm(light)
    fill = np.array([0.6, 0.1, -0.6]) @ turn
    fill /= np.linalg.norm(fill)
    parts = []
    for stuff, pos, nor, col, idx in groups:
        p = (pos - eye) @ turn.T
        front = p[:, 2] > 0.05
        x = cx + focal * p[:, 0] / np.maximum(p[:, 2], 0.05)
        y = cy - focal * p[:, 1] / np.maximum(p[:, 2], 0.05)
        view = -(pos - eye) / np.linalg.norm(pos - eye, axis=1, keepdims=True)
        lit = 0.22 + 0.8 * np.clip(nor @ light, 0, None) + 0.25 * np.clip(nor @ fill, 0, None)
        shade = col[:, :3] * lit[:, None]
        if stuff in METAL + ("Gloss", "Frost"):
            half = light + view
            half /= np.linalg.norm(half, axis=1, keepdims=True)
            shine = np.clip((nor * half).sum(1), 0, None) ** 24
            shade = shade + (col[:, :3] if stuff in METAL else 1.0) * 0.9 * shine[:, None]
        if stuff in ("Frost", "Ember"):
            shade = shade + col[:, :3]
        tris = idx[front[idx].all(axis=1)]
        parts.append((np.stack([x, y], -1)[tris], p[:, 2][tris], shade[tris]))
    xy, z, shade = (np.concatenate(part) for part in zip(*parts))
    pixel, depth, color = fragments(xy, z, shade, width, height)
    order = np.lexsort((depth, pixel))
    pixel, color = pixel[order], color[order]
    first = np.ones(len(pixel), bool)
    first[1:] = pixel[1:] != pixel[:-1]
    image = np.zeros((height * width, 3), np.float32)
    mask = np.zeros(height * width, bool)
    image[pixel[first]] = color[first]
    mask[pixel[first]] = True
    rgb = np.where(image <= 0.0031308, image * 12.92, 1.055 * np.clip(image, 0, None) ** (1 / 2.4) - 0.055)
    return (np.clip(rgb, 0, 1) * 255).astype(np.uint8).reshape(height, width, 3), mask.reshape(height, width)


def reference(spec):
    image = Image.open(spec["image"])
    image = Image.alpha_composite(Image.new("RGBA", image.size, (128, 128, 128, 255)), image.convert("RGBA"))
    left, top, right, bottom = spec.get("crop", [0, 0, 1, 1])
    w, h = image.size
    image = image.crop((int(left * w), int(top * h), int(right * w), int(bottom * h))).convert("RGB")
    return image.resize((round(image.width * HEIGHT / image.height), HEIGHT), Image.LANCZOS)


def sheet(name):
    spec = json.load(open(f"tools/fits/{name}.json"))
    picture = reference(spec)
    width, height = picture.size
    model, mask = render(load(f"screenshots/fit/{name}.bin"), spec.get("camera", {}), width, height)
    back = np.asarray(picture)
    alone = np.where(mask[..., None], model, 24).astype(np.uint8)
    blend = np.where(mask[..., None], (back * 0.5 + model * 0.5), back).astype(np.uint8)
    edge = np.asarray(Image.fromarray(mask.astype(np.uint8) * 255).filter(ImageFilter.FIND_EDGES)) > 0
    edge = np.asarray(Image.fromarray(edge.astype(np.uint8) * 255).filter(ImageFilter.MaxFilter(3))) > 0
    outline = np.where(edge[..., None], np.array([40, 255, 230]), back).astype(np.uint8)
    out = Image.new("RGB", (width * 4, height))
    for index, panel in enumerate([back, alone, blend, outline]):
        out.paste(Image.fromarray(panel), (index * width, 0))
    out.save(f"screenshots/fit/{name}.png")
    print(f"screenshots/fit/{name}.png")


def main():
    names = sys.argv[1:] or sorted(os.path.basename(path)[:-5] for path in glob.glob("tools/fits/*.json"))
    for name in names:
        sheet(name)


if __name__ == "__main__":
    main()
