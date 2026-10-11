import type { CSSProperties } from "react";

/** A faint dot grid that fades out toward the window's edges, under a soft
 *  indigo glow behind the card. Both mix the theme tokens at low alpha, so
 *  dark mode follows `.dark` with no second set of values. */
const DOTS: CSSProperties = {
  backgroundImage:
    "radial-gradient(color-mix(in oklab, var(--color-foreground) 13%, transparent) 1px, transparent 1.5px)",
  backgroundSize: "22px 22px",
  backgroundPosition: "center top",
  maskImage: "radial-gradient(ellipse 75% 70% at 50% 45%, black 20%, transparent 80%)",
  WebkitMaskImage: "radial-gradient(ellipse 75% 70% at 50% 45%, black 20%, transparent 80%)",
};

const GLOW: CSSProperties = {
  backgroundImage: [
    "radial-gradient(ellipse 55% 42% at 50% 38%, color-mix(in oklab, var(--color-brand) 16%, transparent), transparent 70%)",
    "radial-gradient(ellipse 30% 22% at 50% 30%, color-mix(in oklab, var(--color-brand) 10%, transparent), transparent 70%)",
  ].join(", "),
};

export function Backdrop() {
  return (
    <div aria-hidden className="pointer-events-none absolute inset-0 overflow-hidden">
      <div className="absolute inset-0" style={DOTS} />
      <div className="absolute inset-0" style={GLOW} />
    </div>
  );
}
