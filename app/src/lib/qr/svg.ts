/** Draws a QR matrix as one SVG path, a module per unit, with the spec's
 * 4-module quiet zone included in the view box. Runs of dark modules in a row
 * become one rectangle, so a link's code is a few hundred path commands. */

export const QUIET_ZONE = 4;

export interface QrPath {
  /** Width and height of the view box, quiet zone included. */
  extent: number;
  d: string;
}

export function qrPath(modules: readonly (readonly boolean[])[]): QrPath {
  const parts: string[] = [];
  modules.forEach((row, r) => {
    for (let c = 0; c < row.length; c++) {
      if (!row[c]) continue;
      const start = c;
      while (c + 1 < row.length && row[c + 1]) c++;
      parts.push(`M${start + QUIET_ZONE} ${r + QUIET_ZONE}h${c - start + 1}v1h${-(c - start + 1)}z`);
    }
  });
  return { extent: modules.length + QUIET_ZONE * 2, d: parts.join("") };
}
