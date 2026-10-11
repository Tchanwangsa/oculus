import { useCallback, useEffect, useRef, useState } from "react";
import { ArrowClockwise } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { FilterRow } from "@/components/ui/filter/FilterPopover";
import { music, type MusicDevice } from "@/lib/music";
import { DeviceIcon } from "./deviceIcon";
import { MenuGroup } from "./MenuGroup";

/**
 * The account's Spotify Connect devices, the active one checked; a click
 * moves playback there. Asked for once when the menu opens: Rust answers from
 * Spotify's pushed device list, and the refresh asks the Web API, which is
 * rate-limited.
 */
export function SpotifyDevices({ onMoved }: { onMoved: () => void }) {
  const [devices, setDevices] = useState<MusicDevice[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const seq = useRef(0);

  const load = useCallback((refresh: boolean) => {
    const id = ++seq.current;
    setBusy(true);
    setError(null);
    music.devices(refresh).then(
      (list) => {
        if (id !== seq.current) return;
        setDevices(list);
        setBusy(false);
      },
      (e: unknown) => {
        if (id !== seq.current) return;
        setError(String(e));
        setBusy(false);
      },
    );
  }, []);

  useEffect(() => load(false), [load]);

  const move = (device: MusicDevice) => {
    if (device.active) return;
    setError(null);
    setBusy(true);
    music.transfer(device.id).then(onMoved, (e: unknown) => {
      setError(String(e));
      setBusy(false);
    });
  };

  return (
    <MenuGroup
      label="Spotify devices"
      action={
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="Refresh devices"
          disabled={busy}
          onClick={() => load(true)}
        >
          <ArrowClockwise size={12} />
        </Button>
      }
    >
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
