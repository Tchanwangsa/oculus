import type { ReactNode } from "react";

/** The matched run, marked. Split by hand, not regex: the needle is typed text. */
export function Highlight({ text, needle }: { text: string; needle: string }) {
  if (!needle) return <>{text}</>;
  const hay = text.toLowerCase();
  const parts: ReactNode[] = [];
  let at = 0;
  for (;;) {
    const hit = hay.indexOf(needle, at);
    if (hit < 0) {
      parts.push(text.slice(at));
      break;
    }
    if (hit > at) parts.push(text.slice(at, hit));
    parts.push(
      <mark
        key={hit}
        className="bg-brand/20 text-brand rounded-[2px] px-px"
      >
        {text.slice(hit, hit + needle.length)}
      </mark>,
    );
    at = hit + needle.length;
  }
  return <>{parts}</>;
}
