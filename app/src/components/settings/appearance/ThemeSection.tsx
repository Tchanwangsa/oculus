import { useSyncExternalStore } from "react";

import { Section } from "@/components/settings/shared/section";
import { isMac } from "@/lib/platform";
import { isDark, subscribeDark } from "@/lib/ui/themes";
import { AccentRow } from "./AccentRow";
import { FrameRow } from "./FrameRow";
import { ModeRow } from "./ModeRow";
import { ThemeGrid } from "./ThemeGrid";

/** Mode, a theme per mode, the accent and (macOS) the window frame. Titled
 *  like the page, as each settings page's first section is. */
export function ThemeSection() {
  const mode = useSyncExternalStore(subscribeDark, isDark) ? "dark" : "light";
  return (
    <Section title="Appearance" description="How Oculus looks on this machine.">
      <ModeRow />
      <ThemeGrid mode="light" inUse={mode === "light"} />
      <ThemeGrid mode="dark" inUse={mode === "dark"} />
      <AccentRow mode={mode} />
      {isMac && <FrameRow />}
    </Section>
  );
}
