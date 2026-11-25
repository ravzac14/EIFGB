// Turning what a player picks or drops (a folder, a zip, loose files) into
// a project: `{ "game.yaml": …, "scripts/fire.lua": … }`.

import { unzipSync, strFromU8 } from "fflate";
import type { Files } from "./engine.ts";
import { MEDIA, dataUrl } from "./pictures.ts";

const TEXT = /\.(ya?ml|lua)$|(^|\/)walkthrough(-[^/]+)?\.txt$/i;
const SKIP = /(^|\/)(\.|__MACOSX\/)/;

/** Puts `game.yaml` (or the only .yaml file) at the root, keeping paths relative to it. */
export function normalize(raw: Map<string, string>): Files | string {
  const entries = [...raw].map(([p, t]) => [p.replace(/\\/g, "/").replace(/^\/+/, ""), t] as const).filter(([p]) => !SKIP.test(p));
  const yamls = entries.filter(([p]) => /\.ya?ml$/i.test(p));
  const entry =
    yamls.filter(([p]) => /(^|\/)game\.ya?ml$/i.test(p)).sort(([a], [b]) => a.split("/").length - b.split("/").length)[0] ??
    (yamls.length === 1 ? yamls[0] : undefined);
  if (!entry) {
    return yamls.length ? "There's more than one .yaml file, and none is called game.yaml." : "There's no game.yaml in there.";
  }
  const dir = entry[0].includes("/") ? entry[0].slice(0, entry[0].lastIndexOf("/") + 1) : "";
  const files: Files = {};
  for (const [p, t] of entries) {
    if (!p.startsWith(dir) || !(TEXT.test(p) || MEDIA.test(p))) continue;
    files[p === entry[0] ? "game.yaml" : p.slice(dir.length)] = t;
  }
  return files;
}

function unzipInto(bytes: Uint8Array, into: Map<string, string>) {
  const unzipped = unzipSync(bytes, { filter: (f) => TEXT.test(f.name) || MEDIA.test(f.name) });
  for (const [name, data] of Object.entries(unzipped)) into.set(name, MEDIA.test(name) ? dataUrl(name, data) : strFromU8(data));
}

async function fromZip(file: File, into: Map<string, string>) {
  unzipInto(new Uint8Array(await file.arrayBuffer()), into);
}

/** From the bytes of a zip (e.g. downloaded from a link). */
export function fromZipBytes(bytes: Uint8Array): Files | string {
  const raw = new Map<string, string>();
  try {
    unzipInto(bytes, raw);
  } catch {
    return "That isn't a zip file that can be opened.";
  }
  return normalize(raw);
}

/** From an `<input type=file>` (with or without `webkitdirectory`). */
export async function fromFileList(list: FileList | File[]): Promise<Files | string> {
  const raw = new Map<string, string>();
  for (const f of Array.from(list)) {
    if (/\.zip$/i.test(f.name)) await fromZip(f, raw);
    else if (TEXT.test(f.name)) raw.set(f.webkitRelativePath || f.name, await f.text());
    else if (MEDIA.test(f.name)) raw.set(f.webkitRelativePath || f.name, dataUrl(f.name, new Uint8Array(await f.arrayBuffer())));
  }
  return normalize(raw);
}

/** From a drop: files, zips, or whole folders. */
export async function fromDrop(dt: DataTransfer): Promise<Files | string> {
  const raw = new Map<string, string>();
  const walk = async (entry: FileSystemEntry): Promise<void> => {
    if (entry.isFile) {
      const file = await new Promise<File>((ok, err) => (entry as FileSystemFileEntry).file(ok, err));
      const path = entry.fullPath.replace(/^\//, "");
      if (/\.zip$/i.test(file.name)) await fromZip(file, raw);
      else if (TEXT.test(file.name)) raw.set(path, await file.text());
      else if (MEDIA.test(file.name)) raw.set(path, dataUrl(file.name, new Uint8Array(await file.arrayBuffer())));
    } else if (entry.isDirectory) {
      const reader = (entry as FileSystemDirectoryEntry).createReader();
      // readEntries returns batches until it returns none.
      for (;;) {
        const batch = await new Promise<FileSystemEntry[]>((ok, err) => reader.readEntries(ok, err));
        if (!batch.length) break;
        for (const e of batch) await walk(e);
      }
    }
  };
  const entries = Array.from(dt.items)
    .map((i) => i.webkitGetAsEntry?.())
    .filter((e): e is FileSystemEntry => !!e);
  if (entries.length) {
    for (const e of entries) await walk(e);
    return normalize(raw);
  }
  return fromFileList(dt.files);
}
