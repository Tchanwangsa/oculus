import type { ReactNode } from "react";
import type { MusicSnapshot } from "@/lib/music";
import { DeviceMenu } from "./devices/DeviceMenu";
import { MusicVolume } from "./MusicVolume";

/** The volume slider with the device menu at its end, and any extra buttons
 *  (`children`) just before it. */
export function VolumeRow({ snapshot, children }: { snapshot: MusicSnapshot; children?: ReactNode }) {
  return (
    <div className="flex items-center gap-1.5">
      <div className="min-w-0 flex-1">
        <MusicVolume snapshot={snapshot} />
      </div>
      {children}
      <DeviceMenu snapshot={snapshot} />
    </div>
  );
}
