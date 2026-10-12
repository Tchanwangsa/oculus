import type { ThemeSpec } from "@/lib/ui/themes";
import { previewStyle } from "./previewStyle";

/** One bar of the sketch's code line, coloured by a syntax slot. */
const CODE = ["w-3 bg-syntax-keyword", "w-4 bg-syntax-function", "w-1 bg-syntax-operator", "w-5 bg-syntax-string", "w-3 bg-syntax-comment"];

/**
 * The app in miniature, drawn in `theme` (with the accent override): the
 * ground with two tab pills and the rail, and the floating card holding a
 * title, muted lines, a line of code and a button. Token utilities only, no
 * `dark:` variants, so it never follows the page's own mode.
 */
export function ThemeSketch({ theme, accent }: { theme: ThemeSpec; accent: string | null }) {
  return (
    <div aria-hidden style={previewStyle(theme, accent)} className="flex h-[92px] flex-col gap-1 bg-background p-1.5 pt-1">
      <div className="flex h-3 shrink-0 items-center gap-1 pl-4">
        <span className="h-2.5 w-9 rounded-[4px] border border-border bg-card" />
        <span className="h-2.5 w-7 rounded-[4px] bg-sidebar-item-hover" />
      </div>
      <div className="flex min-h-0 flex-1 gap-1">
        <div className="flex w-3 shrink-0 flex-col items-center gap-1 pt-0.5">
          <span className="size-2 rounded-[3px] bg-sidebar-item-active" />
          <span className="size-1.5 rounded-full bg-muted-foreground/50" />
          <span className="size-1.5 rounded-full bg-muted-foreground/50" />
        </div>
        <div className="flex min-w-0 flex-1 flex-col gap-1 rounded-md border border-border bg-card p-1.5 shadow-xs">
          <span className="h-1.5 w-1/2 shrink-0 rounded-full bg-foreground" />
          <span className="h-1 w-full shrink-0 rounded-full bg-muted-foreground/50" />
          <span className="h-1 w-4/5 shrink-0 rounded-full bg-muted-foreground/50" />
          <span className="flex shrink-0 items-center gap-[3px] rounded-[3px] bg-surface-raised px-1 py-[3px]">
            {CODE.map((bar) => (
              <span key={bar} className={`h-1 rounded-full ${bar}`} />
            ))}
          </span>
          <span className="mt-auto flex h-2.5 w-8 shrink-0 items-center justify-center self-end rounded-full bg-primary">
            <span className="h-0.5 w-4 rounded-full bg-primary-foreground" />
          </span>
        </div>
      </div>
    </div>
  );
}
