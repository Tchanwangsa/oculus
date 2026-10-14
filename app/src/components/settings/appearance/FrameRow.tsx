import { setAppearance, useAppearance, type Frame } from "@/lib/ui/themes";
import { AppearanceRow } from "./AppearanceRow";
import { SegmentToggle, type SegmentOption } from "./SegmentToggle";

const OPTIONS: readonly SegmentOption<Frame>[] = [
  { value: "solid", label: "Solid" },
  { value: "frosted", label: "Frosted" },
  { value: "clear", label: "Clear" },
];

/** macOS only: elsewhere the frame is always solid. */
export function FrameRow() {
  const { frame } = useAppearance();
  return (
    <AppearanceRow
      label="Window frame"
      description="Let the desktop show through."
    >
      <SegmentToggle label="Window frame" value={frame} options={OPTIONS} onChange={(next) => setAppearance({ frame: next })} />
    </AppearanceRow>
  );
}
