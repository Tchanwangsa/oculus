import type { ShareEndpoint } from "@/lib/sharing/types";
import { devices } from "@/lib/sharing/view";

/** Which of this Mac's addresses the link and QR use. Labels are the native
 * side's (interface kind and name); a count shows only for devices whose live
 * socket came in through that address, never guessed from the address. */
export function AddressList({ endpoints, selected, onSelect }: {
  endpoints: readonly ShareEndpoint[];
  selected: string | null;
  onSelect: (id: string) => void;
}) {
  const linked = endpoints.filter((e) => e.url);
  if (!linked.length) return <p className="text-xs text-muted-foreground">This Mac has no address another device could use. Join a Wi‑Fi network or a VPN.</p>;
  return (
    <fieldset className="min-w-0">
      <legend className="mb-1 text-xs text-muted-foreground">Address</legend>
      <div className="flex flex-col gap-0.5">
        {linked.map((e) => (
          <label key={e.id} className={`flex min-w-0 cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 transition-[background-color] hover:bg-accent ${e.id === selected ? "bg-accent" : ""}`}>
            <input type="radio" name="share-address" className="shrink-0 accent-primary" checked={e.id === selected} onChange={() => onSelect(e.id)} />
            <span className="flex min-w-0 flex-col">
              <span className="truncate text-xs text-foreground">{e.label}</span>
              <span className="truncate text-[11px] text-muted-foreground tabular-nums" title={e.address}>
                {e.address}{e.connectedClients > 0 && ` · ${devices(e.connectedClients)} connected via this address`}
              </span>
            </span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}
