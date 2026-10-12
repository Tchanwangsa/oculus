import type { SyntaxSlot, ThemeSpec } from "@/lib/ui/themes";
import { cn } from "@/lib/utils";
import { previewStyle } from "./previewStyle";

type Token = readonly [SyntaxSlot | null, string];
interface Line {
  change?: "removed" | "added";
  tokens: readonly Token[];
}

/** Hand-tokenised, so every slot shows without loading a highlighter. */
const SAMPLE: readonly Line[] = [
  {
    tokens: [
      ["keyword", "function"], [null, " "], ["function", "greet"], ["operator", "("], ["property", "name"],
      ["operator", ": "], ["type", "string"], ["operator", ") {"],
    ],
  },
  {
    change: "removed",
    tokens: [
      [null, "  "], ["keyword", "return"], [null, " "], ["string", '"Hello, "'], ["operator", " + "], [null, "name"],
      ["operator", ";"],
    ],
  },
  {
    change: "added",
    tokens: [
      [null, "  "], ["keyword", "return"], [null, " "], ["string", "`Hello, "], ["meta", "${"], [null, "name"],
      ["meta", "}"], ["string", "!`"], ["operator", ";"], [null, " "], ["comment", "// friendlier"],
    ],
  },
  { tokens: [["operator", "}"]] },
];

/** Literal class names, so Tailwind sees each one. */
const SLOT: Record<SyntaxSlot, string> = {
  keyword: "text-syntax-keyword",
  string: "text-syntax-string",
  comment: "text-syntax-comment",
  number: "text-syntax-number",
  function: "text-syntax-function",
  type: "text-syntax-type",
  property: "text-syntax-property",
  operator: "text-syntax-operator",
  meta: "text-syntax-meta",
};

const CHANGE = {
  removed: { row: "bg-destructive/12", mark: "text-destructive", sign: "-" },
  added: { row: "bg-success/12", mark: "text-success", sign: "+" },
} as const;

/**
 * A small diff in `code`'s syntax colours on `app`'s `surface-raised` (code
 * sits on the app theme), in the code font. Inline tokens, so a dark
 * preview stays dark on a light page.
 */
export function CodePreview({ app, code }: { app: ThemeSpec; code: ThemeSpec }) {
  return (
    <div
      aria-hidden
      style={previewStyle(app, null, code.syntax)}
      className="overflow-hidden rounded-lg border border-border bg-surface-raised py-2 font-mono text-[12px] leading-5 text-foreground"
    >
      {SAMPLE.map((line, i) => {
        const change = line.change && CHANGE[line.change];
        return (
          <div key={i} className={cn("flex whitespace-pre pr-3", change?.row)}>
            <span className="w-8 shrink-0 pr-2 text-right text-muted-foreground">{i + 1}</span>
            <span className={cn("w-4 shrink-0", change?.mark)}>{change?.sign ?? " "}</span>
            <span className="min-w-0 truncate">
              {line.tokens.map(([slot, text], j) => (
                <span key={j} className={slot ? SLOT[slot] : undefined}>
                  {text}
                </span>
              ))}
            </span>
          </div>
        );
      })}
    </div>
  );
}
