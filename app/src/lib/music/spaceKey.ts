import { music, musicControllable, shownPlayback, type MusicSnapshot } from "./index";

const OWNS_SPACE =
  'input, textarea, select, [contenteditable="true"], [role="dialog"], [role="menu"], [role="listbox"], [data-slot="popover-content"]';

type SpaceEvent = Pick<KeyboardEvent,
  "key" | "metaKey" | "ctrlKey" | "altKey" | "repeat" | "defaultPrevented" | "target" | "preventDefault" | "stopPropagation"
>;

/** A player's own popover can take Space; fields and nested popups keep it. */
export function handleMusicSpaceKey(
  e: SpaceEvent,
  snapshot: MusicSnapshot | null,
  run: (command: () => Promise<unknown>) => Promise<boolean>,
  scope?: HTMLElement,
) {
  if (e.defaultPrevented || e.key !== " " || e.metaKey || e.ctrlKey || e.altKey) return;
  const owner = (e.target as HTMLElement | null)?.closest(OWNS_SPACE);
  if (owner && owner !== scope) return;
  e.preventDefault();
  e.stopPropagation();
  if (e.repeat || !snapshot || !musicControllable(snapshot)) return;
  void run(shownPlayback(snapshot).playing ? music.pausePlayer : music.play);
}
