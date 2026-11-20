// Pictures in a game: rooms' and things' `picture:`. In a project they're
// kept like any other file, but as `data:` URLs, so a project stays a map of
// text files: it can be stored in the browser, shared and put in a single web
// page. A picture can also be a web address (https://...).

import type { Files } from "./engine.ts";

/** Picture files a project can hold. */
export const IMAGE = /\.(png|jpe?g|gif|webp|svg)$/i;

const TYPES: Record<string, string> = { png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", webp: "image/webp", svg: "image/svg+xml" };

/** The address to show a picture from, or null when it's missing. */
export function pictureSrc(file: string, files: Files): string | null {
  if (/^https:\/\//i.test(file)) return file;
  const v = files[file];
  return v?.startsWith("data:image/") ? v : null;
}

/** A picture file's bytes as a `data:` URL. */
export function dataUrl(name: string, bytes: Uint8Array): string {
  const type = TYPES[name.split(".").pop()?.toLowerCase() ?? ""] ?? "application/octet-stream";
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return `data:${type};base64,${btoa(s)}`;
}

/** A `data:` URL's bytes, for writing the picture out as a file again. */
export function dataBytes(url: string): Uint8Array | null {
  const m = /^data:[^;,]+;base64,(.*)$/.exec(url);
  if (!m) return null;
  const bin = atob(m[1]);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Whether a project file is a picture (kept as a `data:` URL). */
export const isPicture = (path: string, text: string) => IMAGE.test(path) && text.startsWith("data:image/");

/**
 * A picture the author chose, made small enough to keep in the browser:
 * at most 1200 pixels across, as a JPEG (or PNG when it has see-through
 * parts). SVGs and small files are kept as they are.
 */
export async function shrink(file: File): Promise<{ name: string; url: string }> {
  const ext = file.name.split(".").pop()?.toLowerCase() ?? "";
  const bytes = new Uint8Array(await file.arrayBuffer());
  if (ext === "svg" || ext === "gif" || bytes.length < 150_000) return { name: file.name, url: dataUrl(file.name, bytes) };
  const img = await createImageBitmap(new Blob([bytes]));
  const scale = Math.min(1, 1200 / Math.max(img.width, img.height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(img.width * scale);
  canvas.height = Math.round(img.height * scale);
  const g = canvas.getContext("2d")!;
  g.drawImage(img, 0, 0, canvas.width, canvas.height);
  const png = ext === "png" && hasClearParts(g, canvas.width, canvas.height);
  const url = png ? canvas.toDataURL("image/png") : canvas.toDataURL("image/jpeg", 0.82);
  return { name: file.name.replace(/\.[^.]+$/, png ? ".png" : ".jpg"), url };
}

function hasClearParts(g: CanvasRenderingContext2D, w: number, h: number): boolean {
  const data = g.getImageData(0, 0, w, h).data;
  for (let i = 3; i < data.length; i += 4 * 97) if (data[i] < 250) return true;
  return false;
}

/** The pictures a game.yaml names (rooms, objects, characters), as project paths. */
export async function picturesOf(yamlText: string): Promise<string[]> {
  const { parse } = await import("yaml");
  let doc: Record<string, unknown>;
  try {
    doc = parse(yamlText) ?? {};
  } catch {
    return [];
  }
  const out = new Set<string>();
  for (const section of ["rooms", "objects", "characters"]) {
    const s = doc[section];
    if (!s || typeof s !== "object") continue;
    for (const v of Object.values(s as Record<string, unknown>)) {
      const p = (v as { picture?: unknown } | null)?.picture;
      const file = typeof p === "string" ? p : typeof (p as { file?: unknown } | null)?.file === "string" ? (p as { file: string }).file : null;
      if (file && !/^https?:/i.test(file) && IMAGE.test(file)) out.add(file);
    }
  }
  return [...out];
}

/** A picture in the story: the image, with words for those who can't see it. */
export function figure(src: string | null, alt: string): HTMLElement {
  const f = document.createElement("figure");
  f.className = "picture";
  if (src) {
    const img = document.createElement("img");
    img.src = src;
    img.alt = alt;
    img.loading = "lazy";
    f.append(img);
  } else {
    const p = document.createElement("p");
    p.className = "notice";
    p.textContent = `[Picture: ${alt}]`;
    f.append(p);
  }
  return f;
}
