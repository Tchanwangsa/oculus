import type { ReactNode } from "react";
import { InlineMd } from "@/components/markdown/MdComponents";
import { cn } from "@/lib/utils";

/** The card every study widget sits in: the reply's raised surface, a
 *  hairline border, an optional title and a footer row for its controls. */
export function Frame({
  title,
  meta,
  footer,
  children,
  className,
}: {
  title?: string;
  /** Right of the title: a counter or score, in tabular figures. */
  meta?: ReactNode;
  footer?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={cn("my-3 rounded-xl border border-border bg-card px-4 py-3", className)}>
      {(title || meta) && (
        <header className="mb-2 flex flex-wrap items-baseline gap-2">
          {title && <Rich text={title} className="min-w-0 flex-1 basis-36 font-display text-[14px] font-semibold text-foreground" />}
          {meta && <span className="ml-auto shrink-0 text-xs tabular-nums text-muted-foreground">{meta}</span>}
        </header>
      )}
      {children}
      {footer && <footer className="mt-3 flex flex-wrap items-center gap-2">{footer}</footer>}
    </section>
  );
}

/** Model text: inline markdown with `$…$` maths through the app's maths
 *  engine. `InlineMd` drops block elements, so a stray heading can't break
 *  the card, and it is safe inside a `<button>`. */
export function Rich({ text, className }: { text: unknown; className?: string }) {
  if (typeof text !== "string" || !text) return null;
  return <InlineMd text={repairTex(text)} className={className} />;
}

const CONTROL: Record<string, string> = { "\f": "f", "\t": "t", "\b": "b", "\r": "r" };

/**
 * Strings are read with `JSON.parse`, so a single-backslash `\frac`,
 * `\times`, `\beta` or `\rho` arrives as a form feed, tab, backspace or
 * carriage return plus "rac", "imes"…, and `\neq` inside maths as a newline.
 * None of those characters belongs in a widget's text before a letter, so
 * they go back to the TeX command they were.
 */
export function repairTex(text: string): string {
  const fixed = text.replace(/[\f\t\b\r](?=[a-zA-Z])/g, (c) => `\\${CONTROL[c]}`);
  return fixed.replace(/\$[^$]*\$/g, (m) => m.replace(/\n(?=[a-zA-Z])/g, "\\n"));
}

/** A small pill control; the app's `Button` is `h-8`, too heavy inside a card. */
export function Pill({
  children,
  onClick,
  disabled,
  tone = "outline",
  pressed,
}: {
  children: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  tone?: "outline" | "primary" | "ghost";
  pressed?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-pressed={pressed}
      className={cn(
        "inline-flex h-7 cursor-pointer items-center gap-1.5 rounded-full px-3 text-[13px] font-medium transition-[color,background-color,border-color] disabled:cursor-default disabled:opacity-50 [&_svg]:shrink-0",
        tone === "primary" && "bg-primary text-primary-foreground hover:bg-primary-hover",
        tone === "outline" && "border border-border bg-card text-foreground hover:bg-accent",
        tone === "ghost" && "text-muted-foreground hover:bg-accent hover:text-foreground",
      )}
    >
      {children}
    </button>
  );
}
