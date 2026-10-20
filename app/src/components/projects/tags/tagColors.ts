import type { TagColor } from "@/lib/planning/projects";

export interface TagColorClasses {
  /** The 8px dot. */
  dot: string;
  /** The chip's background tint. */
  soft: string;
  /** The chip's text. */
  text: string;
}

/** Class names are written out because Tailwind only emits what it can scan.
 *  The tokens (`--color-tag-*`) live in `app/src/styles/colors.css` and
 *  `dark.css`. */
export const TAG_COLOR_CLASSES: Record<TagColor, TagColorClasses> = {
  slate: { dot: "bg-tag-slate", soft: "bg-tag-slate-soft", text: "text-tag-slate-text" },
  indigo: { dot: "bg-tag-indigo", soft: "bg-tag-indigo-soft", text: "text-tag-indigo-text" },
  green: { dot: "bg-tag-green", soft: "bg-tag-green-soft", text: "text-tag-green-text" },
  amber: { dot: "bg-tag-amber", soft: "bg-tag-amber-soft", text: "text-tag-amber-text" },
  rose: { dot: "bg-tag-rose", soft: "bg-tag-rose-soft", text: "text-tag-rose-text" },
  purple: { dot: "bg-tag-purple", soft: "bg-tag-purple-soft", text: "text-tag-purple-text" },
  teal: { dot: "bg-tag-teal", soft: "bg-tag-teal-soft", text: "text-tag-teal-text" },
  orange: { dot: "bg-tag-orange", soft: "bg-tag-orange-soft", text: "text-tag-orange-text" },
};

/** The colour's classes; a key outside the palette draws as slate. */
export function tagColorClasses(color: string): TagColorClasses {
  return TAG_COLOR_CLASSES[color as TagColor] ?? TAG_COLOR_CLASSES.slate;
}

/** A palette key as words, for labels ("Indigo"). */
export function tagColorLabel(color: TagColor): string {
  return color.charAt(0).toUpperCase() + color.slice(1);
}
