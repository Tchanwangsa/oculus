import { Desktop, Moon, Sun } from "@phosphor-icons/react";

import { setAppearance, useAppearance, type AppearanceMode } from "@/lib/ui/themes";
import { AppearanceRow } from "./AppearanceRow";
import { SegmentToggle, type SegmentOption } from "./SegmentToggle";

const OPTIONS: readonly SegmentOption<AppearanceMode>[] = [
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
  { value: "system", label: "System", icon: Desktop },
];

export function ModeRow() {
  const { mode } = useAppearance();
  return (
    <AppearanceRow
      label="Mode"
      description="System follows your system and changes with it while Oculus is open."
    >
      <SegmentToggle label="Mode" value={mode} options={OPTIONS} onChange={(next) => setAppearance({ mode: next })} />
    </AppearanceRow>
  );
}
