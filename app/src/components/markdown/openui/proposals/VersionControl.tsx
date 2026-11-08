import { ArrowCounterClockwise } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { useProposalVersions } from "./useProposalVersions";

/** Says whether VersionControl draws anything: a version choice exists, the
 * draft differs from its version, or the history failed to load. */
export const showsVersions = (version: ReturnType<typeof useProposalVersions>) => version.versions.length > 1 || version.modified || !!version.error;

/** The V1/V2 select appears only once there is a second version; it sits
 * with the decision, at the footer's left or before a compact card's buttons. */
export function VersionControl({ version, disabled }: { version: ReturnType<typeof useProposalVersions>; disabled: boolean }) {
  if (!showsVersions(version)) return null;
  const off = disabled || version.loading || !!version.error;
  return <div className="flex min-h-6 flex-wrap items-center gap-1.5 text-xs text-muted-foreground">
    {version.versions.length > 1 && <Select value={version.selected.id} onValueChange={version.select} disabled={off}>
      <SelectTrigger size="sm" aria-label="Proposal version" className="h-6 gap-1 rounded-full border-0 bg-surface px-2.5 text-xs tabular-nums shadow-none hover:bg-surface-raised">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {version.versions.map((item, index) => <SelectItem key={item.id} value={item.id}>V{index + 1}</SelectItem>)}
      </SelectContent>
    </Select>}
    {version.modified && <><span role="status">Modified</span><Button variant="ghost" size="xs" disabled={off} aria-label={`Reset to V${version.versions.indexOf(version.selected) + 1}`} onClick={version.reset} className="h-6 gap-1 px-2 text-xs"><ArrowCounterClockwise size={12} />Reset</Button></>}
    {version.error && <span role="alert">Couldn’t load versions. <button type="button" onClick={version.retry} className="cursor-pointer text-brand hover:underline">Try again</button></span>}
  </div>;
}
