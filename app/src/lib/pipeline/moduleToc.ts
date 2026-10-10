/**
 * Parses the module TOC markdown `sync/phases/modules.rs` writes to `modules/NN-slug.md`:
 * `## ` for Canvas SubHeaders, `- [label](href) _(kind)_` for items, indented
 * two spaces per Canvas `indent` level. A video is `_(video <canvas id>)_`,
 * its href where the on-demand download lands (`file_toc_line`).
 */

export interface ModuleItem {
  title: string;
  /** `../files/x.pdf` (in-app), an http(s) URL, or null when not downloaded. */
  href: string | null;
  /** From the trailing `_(kind)_`: quiz | assignment | external | file | video. */
  kind: string | null;
  /** The Canvas file id a `_(video <id>)_` item downloads; null otherwise. */
  canvasFileId: number | null;
  indent: number;
}

interface ModuleSection {
  /** The SubHeader this run of items sits under; null for the leading run. */
  heading: string | null;
  items: ModuleItem[];
}

export interface ModuleToc {
  title: string;
  sections: ModuleSection[];
  itemCount: number;
}

const HEADING = /^\s*##\s+(.*)$/;
const BULLET = /^(\s*)-\s+(.*)$/;
const LINK = /^\[(.+?)\]\((.*?)\)\s*(?:_\((\w+)(?:\s+(\d+))?\)_)?\s*$/;
const BARE = /^(.*?)\s*(?:_\((\w+)(?:\s+(\d+))?\)_)?\s*$/;

const idOf = (digits: string | undefined) => (digits ? Number(digits) : null);

const unescapeMd = (s: string) => s.replace(/\\(.)/g, "$1");

export function parseModuleToc(md: string): ModuleToc {
  const lines = md.split("\n");
  let title = "";
  const sections: ModuleSection[] = [];
  let current: ModuleSection | null = null;
  let itemCount = 0;

  const push = (item: ModuleItem) => {
    if (!current) {
      current = { heading: null, items: [] };
      sections.push(current);
    }
    current.items.push(item);
    itemCount++;
  };

  for (const line of lines) {
    if (!title && line.startsWith("# ")) {
      title = unescapeMd(line.slice(2).trim());
      continue;
    }

    const heading = HEADING.exec(line);
    if (heading) {
      current = { heading: unescapeMd(heading[1].trim()), items: [] };
      sections.push(current);
      continue;
    }

    const bullet = BULLET.exec(line);
    if (!bullet) continue;

    const indent = Math.floor(bullet[1].length / 2);
    const body = bullet[2].trim();

    const link = LINK.exec(body);
    if (link) {
      push({
        title: unescapeMd(link[1]),
        href: link[2] || null,
        kind: link[3] ?? null,
        canvasFileId: idOf(link[4]),
        indent,
      });
      continue;
    }

    const bare = BARE.exec(body);
    push({
      title: unescapeMd(bare?.[1] ?? body),
      href: null,
      kind: bare?.[2] ?? null,
      canvasFileId: idOf(bare?.[3]),
      indent,
    });
  }

  // Drop a leading empty run (module opens straight into a SubHeader).
  return {
    title,
    sections: sections.filter((s) => s.items.length > 0 || s.heading),
    itemCount,
  };
}

/** Resolves a TOC href against the module file that contains it. */
export function resolveTocHref(
  href: string,
  moduleRelPath: string,
): { kind: "internal"; path: string } | { kind: "external"; url: string } {
  if (/^https?:/i.test(href)) return { kind: "external", url: href };

  const dir = moduleRelPath.replace(/[^/]+$/, ""); // "modules/"
  const parts = (dir + href).split("/");
  const out: string[] = [];
  for (const part of parts) {
    if (part === "." || part === "") continue;
    if (part === "..") out.pop();
    else out.push(part);
  }
  return { kind: "internal", path: out.join("/") };
}
