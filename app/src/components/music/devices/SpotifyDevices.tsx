import { useEffect, useState } from "react";
import { FilterRow } from "@/components/ui/filter/FilterPopover";
import { music, type MusicDevice } from "@/lib/music";
import { DeviceIcon } from "./deviceIcon";
import { MenuGroup } from "./MenuGroup";

/**
 * The account's Spotify Connect devices, the active one checked; a click
 * moves playback there. Read once when the menu opens from the device list
 * Spotify last sent: its answer to the connect, then each push.
 */
export function SpotifyDevices({ onMoved }: { onMoved: () => void }) {
  const [devices, setDevices] = useState<MusicDevice[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let live = true;
    music.devices().then(
      (list) => live && setDevices(list),
      (e: unknown) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
  }, []);

  const move = (device: MusicDevice) => {
    if (device.active || busy) return;
    setError(null);
    setBusy(true);
    music.transfer(device.id).then(onMoved, (e: unknown) => {
      setError(String(e));
      setBusy(false);
    });
  };

  return (
    <MenuGroup label="Spotify devices">
      {devices === null && !error && (
        <p className="px-2 py-1.5 text-xs text-muted-foreground">Looking for devices…</p>
      )}
      {devices?.map((d) => (
        <FilterRow
          key={d.id}
          label={d.is_this ? `${d.name} (this Mac)` : d.name}
          hint={d.volume === null ? d.name : `${d.name} · volume ${d.volume}%`}
          icon={<DeviceIcon kind={d.kind} />}
          selected={d.active}
          onPick={() => move(d)}
        />
      ))}
      {error && <p className="px-2 py-1 text-xs text-destructive">{error}</p>}
    </MenuGroup>
  );
}
