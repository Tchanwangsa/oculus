import {
  Car,
  DeviceMobile,
  DeviceTablet,
  Laptop,
  SpeakerHifi,
  SpeakerSimpleHigh,
  Television,
  type Icon,
} from "@phosphor-icons/react";
import type { MusicDevice } from "@/lib/music";

const ICONS: Record<MusicDevice["kind"], Icon> = {
  computer: Laptop,
  phone: DeviceMobile,
  tablet: DeviceTablet,
  speaker: SpeakerHifi,
  tv: Television,
  car: Car,
  other: SpeakerSimpleHigh,
};

/** A Connect device's glyph, by its kind. */
export function DeviceIcon({ kind }: { kind: MusicDevice["kind"] }) {
  const Glyph = ICONS[kind];
  return <Glyph size={14} className="shrink-0" />;
}
