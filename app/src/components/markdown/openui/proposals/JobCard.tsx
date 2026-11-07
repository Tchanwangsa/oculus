import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { DownloadSimple, SpinnerGap } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { proposalJobInfo } from "@/lib/harness/proposals";
import { useProposal } from "./useProposal";
import { CardFrame, useCardStreaming } from "./CardFrame";
import { cardState } from "./cardState";
import { object, type Row } from "./model";

const LABELS: Record<string, string> = { Sync: "Sync", Reparse: "Re-parse", Transcribe: "Transcribe", LectureChapters: "Find lecture chapters", LectureEnd: "Find the lecture end", Download: "Download" };

export function JobCard({ kind, input }: { kind: string; input: Row }) {
  const card = useProposal(kind, input);
  const streaming = useCardStreaming();
  const [info, setInfo] = useState<string>();
  const [error, setError] = useState<string>();
  const source = JSON.stringify(input);
  useEffect(() => {
    let active = true;
    void proposalJobInfo(kind, JSON.parse(source)).then((value) => { if (active) { setInfo(value.description); setError(undefined); } }).catch((e) => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, [kind, source]);
  const job = object(card.state.job);
  const frame = cardState({ streaming, id: card.id, status: card.proposal?.status, busy: card.busy, error: card.error, job: job.phase });
  const pending = !card.proposal || card.proposal.status === "pending";
  const targets = Object.values(input).filter((value) => value != null).map((value) => Array.isArray(value) ? value.join(", ") : typeof value === "boolean" ? value ? "including lectures" : "" : String(value)).filter(Boolean).join(" · ");
  // The job's own error paragraph below carries the message, so the frame gets none.
  const actions = frame.failure === "load"
    ? <Button variant="outline" size="sm" onClick={card.retry}>Try again</Button>
    : pending && <><Button variant="ghost" size="sm" disabled={!frame.editable} onClick={() => card.act("discard")}>Discard</Button><Button size="sm" disabled={!frame.editable || !info || !!error} onClick={() => card.act("start")}>{card.busy && <SpinnerGap className="animate-spin" />}{LABELS[kind]}</Button></>;
  if (frame.collapsed) return <CardFrame label="Job proposal" state={frame} icon={DownloadSimple} title={LABELS[kind]} meta={<Badge variant="secondary">Discarded</Badge>} />;
  return <CardFrame label="Job proposal" state={frame} icon={DownloadSimple} actions={actions}
    title={`${LABELS[kind]}${pending ? "?" : ""}`}
    subtitle={<p className="mt-1 text-xs break-words text-muted-foreground">{targets}</p>}
    status={frame.phase === "applying" ? card.busy ? "Starting…" : "Running…" : frame.phase === "created" ? "Complete" : undefined}>
    <p className="border-t border-border-subtle px-4 py-2 text-[13px] leading-relaxed text-muted-foreground">{info ?? "Loading job details…"}</p>
    {!!(card.error || error || job.error) && <p role="alert" className="px-4 pb-2 text-xs text-destructive">{card.error ?? error ?? String(job.error)}</p>}
  </CardFrame>;
}
