import type { MusicSnapshot } from "@/lib/music";
import { DeviceMenu } from "./devices/DeviceMenu";
import { MusicVolume } from "./MusicVolume";

/** The volume slider with the device menu at its end. */
export function VolumeRow({ snapshot }: { snapshot: MusicSnapshot }) {
  return (
    <div className="flex items-center gap-1.5">
      <div className="min-w-0 flex-1">
        <MusicVolume snapshot={snapshot} />
      </div>
      <DeviceMenu snapshot={snapshot} />
    </div>
  );
}
