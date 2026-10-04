#!/usr/bin/env python3
"""Renders the head meshes dumped by `cargo test --features dev heads -- --ignored`
(screenshots/heads/*.bin) into portrait sheets: front, three-quarter and profile.
Usage: tools/heads.py [name-glob ...]   → screenshots/heads/<name>.png and sheet.png"""
import glob, math, os, struct, sys
import numpy as np
from PIL import Image

STUFFS = ["Skin", "Fur", "Leather", "Cloth", "Iron", "Steel", "Gold", "Bone", "Wood", "Bark",
          "Birch", "Needles", "Stone", "Cliff", "Frost", "Ember", "Gloss", "Membrane", "Scales", "Cinder",
          "Thatch", "Shingle", "Masonry", "Planks"]


def load(path):
    data = open(path, "rb").read()
    (count,), at = struct.unpack_from("<I", data), 4
    groups = []
    for _ in range(count):
        stuff, nv, ni = struct.unpack_from("<III", data, at)
        at += 12
        pos = np.frombuffer(data, "<f4", nv * 3, at).reshape(-1, 3); at += nv * 12
        nor = np.frombuffer(data, "<f4", nv * 3, at).reshape(-1, 3); at += nv * 12
        col = np.frombuffer(data, "<f4", nv * 4, at).reshape(-1, 4); at += nv * 16
        idx = np.frombuffer(data, "<u4", ni, at).reshape(-1, 3); at += ni * 4
        groups.append((STUFFS[stuff], pos, nor, col, idx))
    return groups


def render(groups, yaw, size, center, span):
    c, s = math.cos(yaw), math.sin(yaw)
    turn = np.array([[c, 0, -s], [0, 1, 0], [s, 0, c]], dtype=np.float32)
    depth = np.full((size, size), np.inf, dtype=np.float32)
    image = np.zeros((size, size, 3), dtype=np.float32)
    image[:] = (0.02, 0.02, 0.025)
    key = np.array([-0.5, 0.6, -0.8]); key /= np.linalg.norm(key)
    fill = np.array([0.7, 0.1, -0.5]); fill /= np.linalg.norm(fill)
    rim = np.array([0.2, 0.5, 0.9]); rim /= np.linalg.norm(rim)
    for stuff, pos, nor, col, idx in groups:
        p = (pos - center) @ turn.T
        n = nor @ turn.T
        x = (p[:, 0] / span + 0.5) * size
        y = (0.5 - p[:, 1] / span) * size
        z = p[:, 2]
        lit = 0.18 + 0.85 * np.clip(n @ key, 0, None) + 0.25 * np.clip(n @ fill, 0, None) \
            + 0.3 * np.clip(n @ rim, 0, None)
        shade = col[:, :3] * lit[:, None]
        if stuff in ("Gloss", "Frost"):
            half = key + np.array([0, 0, -1.0]); half /= np.linalg.norm(half)
            shade = shade + 0.6 * np.clip(n @ half, 0, None)[:, None] ** 40
        if stuff == "Frost":
            shade = shade + col[:, :3] * 1.5
        for tri in idx:
            xs, ys = x[tri], y[tri]
            x0, x1 = int(max(0, xs.min())), int(min(size - 1, xs.max() + 1))
            y0, y1 = int(max(0, ys.min())), int(min(size - 1, ys.max() + 1))
            if x0 > x1 or y0 > y1:
                continue
            area = (xs[1] - xs[0]) * (ys[2] - ys[0]) - (xs[2] - xs[0]) * (ys[1] - ys[0])
            if abs(area) < 1e-9:
                continue
            gx, gy = np.meshgrid(np.arange(x0, x1 + 1) + 0.5, np.arange(y0, y1 + 1) + 0.5)
            w0 = ((xs[1] - gx) * (ys[2] - gy) - (xs[2] - gx) * (ys[1] - gy)) / area
            w1 = ((xs[2] - gx) * (ys[0] - gy) - (xs[0] - gx) * (ys[2] - gy)) / area
            w2 = 1 - w0 - w1
            inside = (w0 >= 0) & (w1 >= 0) & (w2 >= 0)
            if not inside.any():
                continue
            zz = w0 * z[tri[0]] + w1 * z[tri[1]] + w2 * z[tri[2]]
            patch = depth[y0:y1 + 1, x0:x1 + 1]
            closer = inside & (zz < patch)
            if not closer.any():
                continue
            patch[closer] = zz[closer]
            color = (w0[..., None] * shade[tri[0]] + w1[..., None] * shade[tri[1]]
                     + w2[..., None] * shade[tri[2]])
            image[y0:y1 + 1, x0:x1 + 1][closer] = color[closer]
    rgb = np.where(image <= 0.0031308, image * 12.92, 1.055 * np.clip(image, 0, None) ** (1 / 2.4) - 0.055)
    return Image.fromarray((np.clip(rgb, 0, 1) * 255).astype(np.uint8))


def portrait(path, size=360):
    groups = load(path)
    body = "farmer" in path or "wight" in path
    top = max(g[1][:, 1].max() for g in groups)
    center = np.array([0, top - (0.9 if body else 0.14), 0], dtype=np.float32)
    span = 2.0 if body else 0.36
    views = [render(groups, yaw, size, center, span) for yaw in (0.0, 0.6, 1.5)]
    sheet = Image.new("RGB", (size * 3, size))
    for index, view in enumerate(views):
        sheet.paste(view, (index * size, 0))
    return sheet


def main():
    names = sys.argv[1:] or ["*"]
    paths = sorted({p for name in names for p in glob.glob(f"screenshots/heads/{name}.bin")})
    sheets = []
    for path in paths:
        sheet = portrait(path)
        sheet.save(path.replace(".bin", ".png"))
        sheets.append(sheet)
    if sheets:
        width, height = sheets[0].size
        columns = 2
        rows = (len(sheets) + columns - 1) // columns
        out = Image.new("RGB", (width * columns, height * rows))
        for index, sheet in enumerate(sheets):
            out.paste(sheet, ((index % columns) * width, (index // columns) * height))
        out.save("screenshots/heads/sheet.png")
        print("screenshots/heads/sheet.png", len(sheets))


if __name__ == "__main__":
    main()
