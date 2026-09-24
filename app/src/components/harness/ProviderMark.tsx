import type { ComponentType, SVGProps } from "react";
import type { Provider } from "@/lib/harness";

/**
 * Vendor marks for the CLI agents the harness drives. All of them are
 * `currentColor` monochrome — the same treatment bb gives them under its
 * per-provider `icons` folders — so they sit in the quiet palette like any
 * other icon and never fight the indigo accent. The caller sizes them with a
 * `size-` utility.
 */

export function ClaudeCodeMark(props: SVGProps<SVGSVGElement>) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 100 100"
      fill="currentColor"
      aria-hidden
      {...props}
    >
      <path d="m19.6 66.5 19.7-11 .3-1-.3-.5h-1l-3.3-.2-11.2-.3L14 53l-9.5-.5-2.4-.5L0 49l.2-1.5 2-1.3 2.9.2 6.3.5 9.5.6 6.9.4L38 49.1h1.6l.2-.7-.5-.4-.4-.4L29 41l-10.6-7-5.6-4.1-3-2-1.5-2-.6-4.2 2.7-3 3.7.3.9.2 3.7 2.9 8 6.1L37 36l1.5 1.2.6-.4.1-.3-.7-1.1L33 25l-6-10.4-2.7-4.3-.7-2.6c-.3-1-.4-2-.4-3l3-4.2L28 0l4.2.6L33.8 2l2.6 6 4.1 9.3L47 29.9l2 3.8 1 3.4.3 1h.7v-.5l.5-7.2 1-8.7 1-11.2.3-3.2 1.6-3.8 3-2L61 2.6l2 2.9-.3 1.8-1.1 7.7L59 27.1l-1.5 8.2h.9l1-1.1 4.1-5.4 6.9-8.6 3-3.5L77 13l2.3-1.8h4.3l3.1 4.7-1.4 4.9-4.4 5.6-3.7 4.7-5.3 7.1-3.2 5.7.3.4h.7l12-2.6 6.4-1.1 7.6-1.3 3.5 1.6.4 1.6-1.4 3.4-8.2 2-9.6 2-14.3 3.3-.2.1.2.3 6.4.6 2.8.2h6.8l12.6 1 3.3 2 1.9 2.7-.3 2-5.1 2.6-6.8-1.6-16-3.8-5.4-1.3h-.8v.4l4.6 4.5 8.3 7.5L89 80.1l.5 2.4-1.3 2-1.4-.2-9.2-7-3.6-3-8-6.8h-.5v.7l1.8 2.7 9.8 14.7.5 4.5-.7 1.4-2.6 1-2.7-.6-5.8-8-6-9-4.7-8.2-.5.4-2.9 30.2-1.3 1.5-3 1.2-2.5-2-1.4-3 1.4-6.2 1.6-8 1.3-6.4 1.2-7.9.7-2.6v-.2H49L43 72l-9 12.3-7.2 7.6-1.7.7-3-1.5.3-2.8L24 86l10-12.8 6-7.9 4-4.6-.1-.5h-.3L17.2 77.4l-4.7.6-2-2 .2-3 1-1 8-5.5Z" />
    </svg>
  );
}

export function CodexMark(props: SVGProps<SVGSVGElement>) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="1.68 1.75 16.65 16.5"
      fill="currentColor"
      aria-hidden
      {...props}
    >
      <path d="M11.248 18.25q-.825 0-1.568-.314a4.3 4.3 0 0 1-1.32-.874 4 4 0 0 1-1.304.214 4 4 0 0 1-2.046-.544 4.27 4.27 0 0 1-1.518-1.485 4 4 0 0 1-.56-2.095q0-.48.131-1.04A4.4 4.4 0 0 1 2.04 10.71a4.07 4.07 0 0 1 .017-3.4 4.2 4.2 0 0 1 1.056-1.418 3.8 3.8 0 0 1 1.6-.842 3.9 3.9 0 0 1 .76-1.683q.593-.759 1.451-1.188a4.04 4.04 0 0 1 1.832-.429q.825 0 1.567.313.742.314 1.32.875a4 4 0 0 1 1.304-.215q1.106 0 2.046.545a4.14 4.14 0 0 1 1.501 1.485q.578.941.578 2.095 0 .48-.132 1.04.66.61 1.023 1.419.363.792.363 1.666 0 .892-.38 1.717a4.3 4.3 0 0 1-1.072 1.435 3.8 3.8 0 0 1-1.584.825 3.8 3.8 0 0 1-.775 1.683 4.06 4.06 0 0 1-1.436 1.188 4.04 4.04 0 0 1-1.832.429m-4.076-2.062q.825 0 1.435-.347l3.103-1.782a.36.36 0 0 0 .164-.313v-1.42L7.881 14.62a.67.67 0 0 1-.726 0l-3.118-1.798a.5.5 0 0 1-.017.115v.198q0 .841.396 1.551.413.693 1.139 1.089a3.2 3.2 0 0 0 1.617.412m.165-2.69a.4.4 0 0 0 .181.05q.083 0 .165-.05l1.238-.71-3.977-2.31a.7.7 0 0 1-.363-.643v-3.58q-.825.362-1.32 1.122a2.9 2.9 0 0 0-.495 1.65q0 .809.413 1.55.412.743 1.072 1.123zm3.91 3.663q.875 0 1.585-.396a2.96 2.96 0 0 0 1.534-2.64v-3.564a.32.32 0 0 0-.165-.297l-1.254-.726v4.604a.7.7 0 0 1-.363.643l-3.119 1.799a3 3 0 0 0 1.783.577m.627-6.039V8.878L10.01 7.822 8.129 8.878v2.244l1.881 1.056zM7.057 5.859a.7.7 0 0 1 .363-.644l3.119-1.798a3 3 0 0 0-1.782-.578q-.874 0-1.584.396A2.96 2.96 0 0 0 6.05 4.324a3.07 3.07 0 0 0-.396 1.551v3.547q0 .199.165.314l1.237.726zm8.383 7.887q.825-.364 1.303-1.123.495-.758.495-1.65a3.15 3.15 0 0 0-.412-1.55q-.413-.743-1.073-1.123l-3.086-1.782q-.099-.065-.181-.049a.3.3 0 0 0-.165.05l-1.238.692 3.993 2.327a.6.6 0 0 1 .264.264.64.64 0 0 1 .1.363zm-3.317-8.382a.63.63 0 0 1 .726 0l3.135 1.831v-.297q0-.792-.396-1.501a2.86 2.86 0 0 0-1.105-1.155q-.71-.43-1.65-.43-.825 0-1.436.347L8.294 5.941a.36.36 0 0 0-.165.314v1.418z" />
    </svg>
  );
}

/**
 * opencode's own mark, which is the only two-tone one of the three: a heavy
 * rectangular frame with a block filling the lower two-thirds of its opening —
 * a cursor sitting in a terminal.
 *
 * **Both halves are `currentColor`; the inner block is the same ink at a
 * fraction of its alpha.** The vendor ships a light and a dark file, and the
 * difference between them is only that each is composed against its own
 * background: measured off the two, the inner block is the outer colour at
 * α≈0.21 over the page in both. Carrying two SVGs to say that would also cost
 * the thing every mark here has — one colour, inherited — so the ratio is
 * expressed as an opacity and the theme takes care of itself. It is set a
 * little heavier than measured because these are drawn at 14px in ink that is
 * already `muted-foreground` at 60–70% opacity, where a true 21% block
 * disappears into the surface.
 *
 * Its viewBox is 4:5 where the other two are square, so it letterboxes inside
 * the caller's `size-` box rather than being stretched to match them.
 */
export function OpencodeMark(props: SVGProps<SVGSVGElement>) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 240 300"
      fill="currentColor"
      aria-hidden
      {...props}
    >
      <path fillRule="evenodd" d="M0 0h240v300H0V0Zm60 60v180h120V60H60Z" />
      <path d="M60 120h120v120H60z" fillOpacity={0.35} />
    </svg>
  );
}

/**
 * Antigravity's mark: the vendor's own arch, a single swept "A" whose legs
 * flare out and curl up at the feet.
 *
 * One `currentColor` path like Claude's and Codex's rather than opencode's
 * two-tone treatment, because the vendor's mark is a single weight with no
 * second value in it to express. The vendor file is filled white; that is
 * swapped for `currentColor` so the theme colours it like the others.
 *
 * Its viewBox is 23:21 rather than square, so like opencode's it letterboxes
 * inside the caller's `size-` box.
 */
export function AntigravityMark(props: SVGProps<SVGSVGElement>) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 23 21"
      fill="currentColor"
      aria-hidden
      {...props}
    >
      <path d="M20.111 19.9772C21.3499 20.9064 23.2083 20.2869 21.5048 18.5834C16.3943 13.6278 17.4784 0 11.129 0C4.7797 0 5.86372 13.6278 0.753279 18.5834C-1.10506 20.4418 0.908132 20.9064 2.14704 19.9772C6.94777 16.7251 6.63804 10.9952 11.129 10.9952C15.62 10.9952 15.3103 16.7251 20.111 19.9772Z" />
    </svg>
  );
}

/**
 * Every provider's mark, exhaustively.
 *
 * A `Record<Provider, …>` rather than a ternary on purpose: a ternary answered
 * "not Claude" with the Codex mark, so a third agent would have drawn itself
 * as Codex with nothing to catch it. Adding a provider to the union is a
 * compile error here until its mark exists.
 */
const MARKS: Record<Provider, ComponentType<SVGProps<SVGSVGElement>>> = {
  claude: ClaudeCodeMark,
  codex: CodexMark,
  opencode: OpencodeMark,
  antigravity: AntigravityMark,
};

/** The mark for a provider — the tab strip, the trigger and the model rows
 *  all use it, so a thread's agent is readable at a glance. */
export function ProviderMark({ provider, className }: { provider: Provider; className?: string }) {
  const Mark = MARKS[provider];
  return <Mark className={className} />;
}
