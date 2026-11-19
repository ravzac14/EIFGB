// Room colours, for grouping rooms into regions on the editor's map and the
// player's map. Kept in map.yaml (`colours: { kitchen: green }`), so the
// game itself never sees them.

/**
 * The colours a room can have, by name.
 */
export const COLOURS: Record<string, { label: string; hue: number; sat: number }> = {
  red: { label: "Red", hue: 0, sat: 65 },
  orange: { label: "Orange", hue: 28, sat: 75 },
  yellow: { label: "Yellow", hue: 50, sat: 75 },
  green: { label: "Green", hue: 125, sat: 45 },
  teal: { label: "Teal", hue: 175, sat: 50 },
  blue: { label: "Blue", hue: 212, sat: 65 },
  purple: { label: "Purple", hue: 270, sat: 50 },
  pink: { label: "Pink", hue: 330, sat: 60 },
  brown: { label: "Brown", hue: 28, sat: 35 },
  grey: { label: "Grey", hue: 0, sat: 0 },
};
/**
 * A room colour as CSS custom properties, `--tint` (a see-through fill, so
 * it works in light and dark themes) and `--tint-line`.
 */
export function colourStyle(name: string | undefined): string {
  const c = name ? COLOURS[name] : undefined;
  return c ? `--tint: hsl(${c.hue} ${c.sat}% 52% / 0.28); --tint-line: hsl(${c.hue} ${c.sat}% 48%)` : "";
}
