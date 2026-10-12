import { useEffect, useState } from "react";
import { FilterRow } from "@/components/ui/filter/FilterPopover";
import { music, type MusicOutput } from "@/lib/music";
import { useMusicPrefsStore } from "@/stores/music/musicPrefsStore";
import { MenuGroup } from "./MenuGroup";

/**
 * Where this Mac plays Spotify: the system default, which follows the Mac's
 * own output, or one device by name. A chosen device that is not connected
 * stays listed, checked, so the choice is visible while Rust plays on the
 * default.
 */
export function OutputDevices({ chosen }: { chosen: string | null }) {
  const setOutput = useMusicPrefsStore((s) => s.setOutput);
  const [outputs, setOutputs] = useState<MusicOutput[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    music.outputs().then(
      (list) => live && setOutputs(list),
      (e: unknown) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
  }, []);

  const pick = (name: string | null) => {
    setError(null);
    setOutput(name).catch((e: unknown) => setError(String(e)));
  };
  const current = outputs?.find((o) => o.is_default)?.name;
  const missing = chosen !== null && outputs !== null && !outputs.some((o) => o.name === chosen);

  return (
    <MenuGroup label="Output on this Mac">
      <FilterRow
        label={current ? `System default (${current})` : "System default"}
        selected={chosen === null}
        onPick={() => pick(null)}
      />
      {outputs?.map((o) => (
        <FilterRow
          key={o.name}
          label={o.name}
          selected={chosen === o.name}
          onPick={() => pick(o.name)}
        />
      ))}
      {missing && (
        <FilterRow label={`${chosen} (not connected)`} selected onPick={() => pick(chosen)} />
      )}
      {error && <p className="px-2 py-1 text-xs text-destructive">{error}</p>}
    </MenuGroup>
  );
}
