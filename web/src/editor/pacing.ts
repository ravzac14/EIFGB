// The pacing chart: along the walkthrough, step by step, where new rooms,
// new things, points, facts and goals come, and the stretches where
// nothing does. A long dry stretch is where a player might be bored, or
// lost.

import type { Beat } from "../engine.ts";
import { el } from "./ui.ts";

/** Steps in a row with nothing new, before it's pointed out. */
export const DRY = 8;

const ROWS: { label: string; has: (b: Beat) => string[] }[] = [
  { label: "New rooms", has: (b) => b.rooms },
  { label: "New things", has: (b) => b.things },
  { label: "Points", has: (b) => (b.score ? [`${b.score > 0 ? "+" : ""}${b.score} points`] : []) },
  { label: "Facts and goals", has: (b) => [...b.learned.map((f) => `learned: ${f}`), ...b.goals.map((g) => `done: ${g}`)] },
];

/** What a step brought, in words. */
export function brought(b: Beat): string[] {
  return [...b.rooms.map((r) => `reached ${r}`), ...b.things.map((t) => `got the ${t}`), ...ROWS[2].has(b), ...ROWS[3].has(b), ...(b.ending ? [`the ${b.ending.replace(/_/g, " ")} ending`] : [])];
}

/** Stretches of DRY or more steps that brought nothing: [first, last] step indexes. */
export function dryStretches(beats: Beat[]): [number, number][] {
  const out: [number, number][] = [];
  let from = -1;
  beats.forEach((b, i) => {
    const empty = !brought(b).length;
    if (empty && from < 0) from = i;
    if ((!empty || i === beats.length - 1) && from >= 0) {
      const to = empty ? i : i - 1;
      if (to - from + 1 >= DRY) out.push([from, to]);
      from = -1;
    }
  });
  return out;
}

const SVG = "http://www.w3.org/2000/svg";
const svg = (tag: string, attrs: Record<string, string | number>) => {
  const n = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) n.setAttribute(k, String(v));
  return n;
};

/** The chart, its hover, and a table of the same. */
export function pacingChart(beats: Beat[], onStep?: (i: number) => void): HTMLElement {
  const LEFT = 112;
  const STEP = 14;
  const ROW = 24;
  const TOP = 22;
  const width = LEFT + Math.max(beats.length, 10) * STEP + 12;
  const height = TOP + ROWS.length * ROW + 22;
  const chart = svg("svg", { width, height, viewBox: `0 0 ${width} ${height}`, role: "img", "aria-label": `Pacing along the walkthrough: ${beats.length} steps` });
  const x = (i: number) => LEFT + i * STEP + STEP / 2;
  // Dry stretches, behind everything, each said in words above it.
  for (const [a, b] of dryStretches(beats)) {
    chart.append(
      svg("rect", { x: LEFT + a * STEP, y: TOP - 4, width: (b - a + 1) * STEP, height: ROWS.length * ROW + 4, rx: 4, class: "pace-dry" }),
      Object.assign(svg("text", { x: LEFT + a * STEP + 4, y: TOP - 8, class: "pace-note" }), { textContent: `${b - a + 1} steps, nothing new` }),
    );
  }
  ROWS.forEach((r, j) => {
    const y = TOP + j * ROW + ROW / 2;
    chart.append(
      svg("line", { x1: LEFT, x2: width - 8, y1: y, y2: y, class: "pace-grid" }),
      Object.assign(svg("text", { x: 0, y: y + 4, class: "pace-label" }), { textContent: r.label }),
    );
    beats.forEach((b, i) => {
      if (r.has(b).length) chart.append(svg("circle", { cx: x(i), cy: y, r: 5, class: "pace-mark" }));
    });
  });
  // Step numbers, every ten.
  for (let i = 0; i < beats.length; i += 10) {
    chart.append(Object.assign(svg("text", { x: x(i), y: height - 6, class: "pace-axis", "text-anchor": "middle" }), { textContent: String(i + 1) }));
  }
  const end = beats.findIndex((b) => b.ending);
  if (end >= 0) chart.append(Object.assign(svg("text", { x: x(end), y: TOP - 8, class: "pace-note", "text-anchor": "end" }), { textContent: "the end" }));
  // Hover: a column for each step, wider than its marks.
  const tip = el("div", { className: "pace-tip", hidden: true, role: "status" });
  const cross = svg("line", { x1: 0, x2: 0, y1: TOP - 4, y2: TOP + ROWS.length * ROW, class: "pace-cross", visibility: "hidden" });
  chart.append(cross);
  beats.forEach((b, i) => {
    const hit = svg("rect", { x: LEFT + i * STEP, y: 0, width: STEP, height, class: "pace-hit" });
    const show = () => {
      cross.setAttribute("x1", String(x(i)));
      cross.setAttribute("x2", String(x(i)));
      cross.setAttribute("visibility", "visible");
      const what = brought(b);
      tip.replaceChildren(el("strong", { textContent: `${i + 1}. ${b.command.toUpperCase()}` }), el("div", { textContent: `In ${b.room}` }), el("div", { className: "dim", textContent: what.length ? what.join("; ") : "Nothing new" }));
      tip.hidden = false;
      tip.style.left = `${Math.min(x(i) + 10, width - 220)}px`;
    };
    hit.addEventListener("mouseenter", show);
    hit.addEventListener("focus", show);
    hit.addEventListener("mouseleave", () => {
      tip.hidden = true;
      cross.setAttribute("visibility", "hidden");
    });
    if (onStep) hit.addEventListener("click", () => onStep(i));
    chart.append(hit);
  });
  const table = el(
    "table",
    { className: "pace-table" },
    el("thead", {}, el("tr", {}, el("th", { textContent: "Step" }), el("th", { textContent: "Command" }), el("th", { textContent: "Room" }), el("th", { textContent: "What it brought" }))),
    el("tbody", {}, ...beats.map((b, i) => el("tr", {}, el("td", { textContent: String(i + 1) }), el("td", { textContent: b.command }), el("td", { textContent: b.room }), el("td", { textContent: brought(b).join("; ") || "—" })))),
  );
  return el("div", { className: "pacing" }, el("div", { className: "pace-scroll" }, chart, tip), el("details", {}, el("summary", { textContent: "As a table" }), table));
}
