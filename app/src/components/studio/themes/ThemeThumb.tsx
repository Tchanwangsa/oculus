import { tokenAt, type Tokens } from "@/lib/studio/themeDraft";
import { cn } from "@/lib/utils";

const str = (tokens: Tokens | null, path: string[], fallback: string) => {
  const v = tokenAt(tokens, path);
  return typeof v === "string" ? v : fallback;
};

/**
 * A theme in miniature, drawn in CSS from its tokens (so an unsaved edit
 * shows at once): the canvas, a heading in the heading font and text colour,
 * a muted line, an accent pill and a surface card with its corner radius.
 */
export function ThemeThumb({ tokens, className }: { tokens: Tokens | null; className?: string }) {
  const radius = tokenAt(tokens, ["radius", "m"]);
  const r = typeof radius === "number" ? Math.min(radius / 6, 8) : 3;
  return (
    <div
      aria-hidden
      className={cn("relative aspect-video w-full overflow-hidden rounded-md ring-1 ring-border", className)}
      style={{ backgroundColor: str(tokens, ["color", "bg"], "var(--color-secondary)") }}
    >
      <div className="absolute inset-x-[9%] top-[16%] space-y-[5%]">
        <div
          className="truncate text-[13px] font-bold leading-none"
          style={{
            color: str(tokens, ["color", "text"], "transparent"),
            fontFamily: str(tokens, ["font", "heading"], "inherit"),
          }}
        >
          Aa
        </div>
        <div className="h-[3px] w-1/2 rounded-full" style={{ backgroundColor: str(tokens, ["color", "muted"], "transparent") }} />
        <div className="h-[6px] w-1/4 rounded-full" style={{ backgroundColor: str(tokens, ["color", "accent"], "transparent") }} />
      </div>
      <div
        className="absolute bottom-[14%] right-[9%] h-[34%] w-[34%]"
        style={{
          backgroundColor: str(tokens, ["color", "surface"], "transparent"),
          borderRadius: r,
          boxShadow: `0 0 0 1px ${str(tokens, ["color", "border"], "transparent")}`,
        }}
      />
    </div>
  );
}
