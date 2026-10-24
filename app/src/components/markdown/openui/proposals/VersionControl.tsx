import { ArrowCounterClockwise } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { useProposalVersions } from "./useProposalVersions";

export function VersionControl({ version, disabled }: { version: ReturnType<typeof useProposalVersions>; disabled: boolean }) {
  return <div className="mt-1 flex min-h-6 flex-wrap items-center gap-1.5 text-[11px] text-muted-foreground">
    <Select value={version.selected.id} onValueChange={version.select} disabled={disabled || version.loading || !!version.error}>
      <SelectTrigger size="sm" aria-label="Proposal version" className="h-6 gap-1 border-0 bg-surface px-2 text-[11px] shadow-none">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {version.versions.map((item, index) => <SelectItem key={item.id} value={item.id}>V{index + 1}</SelectItem>)}
      </SelectContent>
    </Select>
    {version.modified && <><span role="status">Modified</span><Button variant="ghost" size="xs" disabled={disabled || version.loading || !!version.error} aria-label={`Reset to V${version.versions.indexOf(version.selected) + 1}`} onClick={version.reset} className="h-6 gap-1 px-2 text-[11px]"><ArrowCounterClockwise size={12} />Reset</Button></>}
    {version.error && <span role="alert">Couldn’t load versions. <button type="button" onClick={version.retry} className="cursor-pointer text-brand hover:underline">Try again</button></span>}
  </div>;
}
