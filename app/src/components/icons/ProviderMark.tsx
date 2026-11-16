import type { ReactNode, SVGProps } from "react";
import type { Provider } from "@/lib/harness";
import { cn } from "@/lib/utils";
import antigravityUrl from "@/assets/providers/antigravity.svg";
import opencodeLightUrl from "@/assets/providers/opencode-light.svg";
import opencodeDarkUrl from "@/assets/providers/opencode-dark.svg";

/** Claude's clay, from the vendor's mark (`assets/providers/claude.svg`). */
export const CLAUDE_FILL = "#D97757";

const CLAUDE_PATH =
  "m19.6 66.5 19.7-11 .3-1-.3-.5h-1l-3.3-.2-11.2-.3L14 53l-9.5-.5-2.4-.5L0 49l.2-1.5 2-1.3 2.9.2 6.3.5 9.5.6 6.9.4L38 49.1h1.6l.2-.7-.5-.4-.4-.4L29 41l-10.6-7-5.6-4.1-3-2-1.5-2-.6-4.2 2.7-3 3.7.3.9.2 3.7 2.9 8 6.1L37 36l1.5 1.2.6-.4.1-.3-.7-1.1L33 25l-6-10.4-2.7-4.3-.7-2.6c-.3-1-.4-2-.4-3l3-4.2L28 0l4.2.6L33.8 2l2.6 6 4.1 9.3L47 29.9l2 3.8 1 3.4.3 1h.7v-.5l.5-7.2 1-8.7 1-11.2.3-3.2 1.6-3.8 3-2L61 2.6l2 2.9-.3 1.8-1.1 7.7L59 27.1l-1.5 8.2h.9l1-1.1 4.1-5.4 6.9-8.6 3-3.5L77 13l2.3-1.8h4.3l3.1 4.7-1.4 4.9-4.4 5.6-3.7 4.7-5.3 7.1-3.2 5.7.3.4h.7l12-2.6 6.4-1.1 7.6-1.3 3.5 1.6.4 1.6-1.4 3.4-8.2 2-9.6 2-14.3 3.3-.2.1.2.3 6.4.6 2.8.2h6.8l12.6 1 3.3 2 1.9 2.7-.3 2-5.1 2.6-6.8-1.6-16-3.8-5.4-1.3h-.8v.4l4.6 4.5 8.3 7.5L89 80.1l.5 2.4-1.3 2-1.4-.2-9.2-7-3.6-3-8-6.8h-.5v.7l1.8 2.7 9.8 14.7.5 4.5-.7 1.4-2.6 1-2.7-.6-5.8-8-6-9-4.7-8.2-.5.4-2.9 30.2-1.3 1.5-3 1.2-2.5-2-1.4-3 1.4-6.2 1.6-8 1.3-6.4 1.2-7.9.7-2.6v-.2H49L43 72l-9 12.3-7.2 7.6-1.7.7-3-1.5.3-2.8L24 86l10-12.8 6-7.9 4-4.6-.1-.5h-.3L17.2 77.4l-4.7.6-2-2 .2-3 1-1 8-5.5Z";

/** OpenAI's blossom (`assets/providers/openai.svg`), which has no fill of its own. */
const OPENAI_PATH =
  "m297.06 130.97c7.26-21.79 4.76-45.66-6.85-65.48-17.46-30.4-52.56-46.04-86.84-38.68-15.25-17.18-37.16-26.95-60.13-26.81-35.04-.08-66.13 22.48-76.91 55.82-22.51 4.61-41.94 18.7-53.31 38.67-17.59 30.32-13.58 68.54 9.92 94.54-7.26 21.79-4.76 45.66 6.85 65.48 17.46 30.4 52.56 46.04 86.84 38.68 15.24 17.18 37.16 26.95 60.13 26.8 35.06.09 66.16-22.49 76.94-55.86 22.51-4.61 41.94-18.7 53.31-38.67 17.57-30.32 13.55-68.51-9.94-94.51zm-120.28 168.11c-14.03.02-27.62-4.89-38.39-13.88.49-.26 1.34-.73 1.89-1.07l63.72-36.8c3.26-1.85 5.26-5.32 5.24-9.07v-89.83l26.93 15.55c.29.14.48.42.52.74v74.39c-.04 33.08-26.83 59.9-59.91 59.97zm-128.84-55.03c-7.03-12.14-9.56-26.37-7.15-40.18.47.28 1.3.79 1.89 1.13l63.72 36.8c3.23 1.89 7.23 1.89 10.47 0l77.79-44.92v31.1c.02.32-.13.63-.38.83l-64.41 37.19c-28.69 16.52-65.33 6.7-81.92-21.95zm-16.77-139.09c7-12.16 18.05-21.46 31.21-26.29 0 .55-.03 1.52-.03 2.2v73.61c-.02 3.74 1.98 7.21 5.23 9.06l77.79 44.91-26.93 15.55c-.27.18-.61.21-.91.08l-64.42-37.22c-28.63-16.58-38.45-53.21-21.95-81.89zm221.26 51.49-77.79-44.92 26.93-15.54c.27-.18.61-.21.91-.08l64.42 37.19c28.68 16.57 38.51 53.26 21.94 81.94-7.01 12.14-18.05 21.44-31.2 26.28v-75.81c.03-3.74-1.96-7.2-5.2-9.06zm26.8-40.34c-.47-.29-1.3-.79-1.89-1.13l-63.72-36.8c-3.23-1.89-7.23-1.89-10.47 0l-77.79 44.92v-31.1c-.02-.32.13-.63.38-.83l64.41-37.16c28.69-16.55 65.37-6.7 81.91 22 6.99 12.12 9.52 26.31 7.15 40.1zm-168.51 55.43-26.94-15.55c-.29-.14-.48-.42-.52-.74v-74.39c.02-33.12 26.89-59.96 60.01-59.94 14.01 0 27.57 4.92 38.34 13.88-.49.26-1.33.73-1.89 1.07l-63.72 36.8c-3.26 1.85-5.26 5.31-5.24 9.06l-.04 89.79zm14.63-31.54 34.65-20.01 34.65 20v40.01l-34.65 20-34.65-20z";

const ANTIGRAVITY_PATH =
  "M20.111 19.9772C21.3499 20.9064 23.2083 20.2869 21.5048 18.5834C16.3943 13.6278 17.4784 0 11.129 0C4.7797 0 5.86372 13.6278 0.753279 18.5834C-1.10506 20.4418 0.908132 20.9064 2.14704 19.9772C6.94777 16.7251 6.63804 10.9952 11.129 10.9952C15.62 10.9952 15.3103 16.7251 20.111 19.9772Z";

function Svg({ viewBox, className, children, ...rest }: SVGProps<SVGSVGElement>) {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" viewBox={viewBox} fill="currentColor" aria-hidden className={className} {...rest}>
      {children}
    </svg>
  );
}

/** A full-colour vendor file. An `<img>`, not inline SVG: inline, every row
 *  would repeat the file's mask and filter ids, and the first copy's defs win
 *  or vanish when that row unmounts. */
function Img({ src, className }: { src: string; className?: string }) {
  return <img src={src} alt="" aria-hidden draggable={false} className={cn("shrink-0 select-none", className)} />;
}

/** The vendor's colours: Claude's clay, Antigravity's gradient arch,
 *  OpenAI's blossom in the full foreground (never muted), opencode's light
 *  or dark file following the `.dark` class. */
function colourMark(provider: Provider, className?: string): ReactNode {
  switch (provider) {
    case "claude":
      return (
        <Svg viewBox="0 0 100 100" fill={CLAUDE_FILL} className={className}>
          <path d={CLAUDE_PATH} />
        </Svg>
      );
    case "codex":
      return (
        <Svg viewBox="0 0 320 320" className={cn(className, "text-foreground")}>
          <path d={OPENAI_PATH} />
        </Svg>
      );
    case "antigravity":
      return <Img src={antigravityUrl} className={className} />;
    case "opencode":
      return (
        <>
          <Img src={opencodeLightUrl} className={cn(className, "dark:hidden")} />
          <Img src={opencodeDarkUrl} className={cn(className, "hidden dark:block")} />
        </>
      );
  }
}

/** Every mark in `currentColor`, for a spot whose tone is the caller's. */
function monoMark(provider: Provider, className?: string): ReactNode {
  switch (provider) {
    case "claude":
      return (
        <Svg viewBox="0 0 100 100" className={className}>
          <path d={CLAUDE_PATH} />
        </Svg>
      );
    case "codex":
      return (
        <Svg viewBox="0 0 320 320" className={className}>
          <path d={OPENAI_PATH} />
        </Svg>
      );
    case "antigravity":
      return (
        <Svg viewBox="0 0 23 21" className={className}>
          <path d={ANTIGRAVITY_PATH} />
        </Svg>
      );
    case "opencode":
      // The vendor's two files differ only in the inner block's weight.
      return (
        <Svg viewBox="0 0 240 300" className={className}>
          <path fillRule="evenodd" d="M0 0h240v300H0V0Zm60 60v180h120V60H60Z" />
          <path d="M60 120h120v120H60z" fillOpacity={0.35} />
        </Svg>
      );
  }
}

/** A CLI agent's mark, in its vendor's colours. `mono` draws it in
 *  `currentColor` instead, for a spot that has to tint it. Sized by the
 *  caller's `size-` utility. */
export function ProviderMark({
  provider,
  className,
  mono = false,
}: {
  provider: Provider;
  className?: string;
  mono?: boolean;
}) {
  return <>{mono ? monoMark(provider, className) : colourMark(provider, className)}</>;
}
