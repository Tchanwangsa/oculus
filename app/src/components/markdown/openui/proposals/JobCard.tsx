import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { DownloadSimple, SpinnerGap, ArrowsClockwise, FileText, Microphone, ListNumbers, FlagCheckered } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { proposalJobInfo } from "@/lib/harness/proposals";
import { useSubjects } from "@/hooks/data/useSubjects";
import { displayCode } from "@/lib/format/format";
import { JobProgress } from "./JobProgress.tsx";
import { useProposal } from "./useProposal";
import { CardFrame, useCardStreaming } from "./CardFrame";
import { cardState } from "./cardState";
import { object, type Row } from "./model";

const LABELS: Record<string, string> = { Sync: "Sync", Reparse: "Re-parse", Transcribe: "Transcribe", LectureChapters: "Find lecture chapters", LectureEnd: "Find the lecture end", Download: "Download" };

export function JobCard({ kind, input }: { kind: string; input: Row }) {
  const card = useProposal(kind, input);
  const streaming = useCardStreaming();
  const { subjects } = useSubjects();
  const Icon = { Sync: ArrowsClockwise, Reparse: FileText, Transcribe: Microphone, LectureChapters: ListNumbers, LectureEnd: FlagCheckered, Download: DownloadSimple }[kind] ?? DownloadSimple;
  const [info, setInfo] = useState<string>();
  const [error, setError] = useState<string>();
  const source = JSON.stringify(input);
  useEffect(() => {
    let active = true;
    setInfo(undefined);
    setError(undefined);
    void proposalJobInfo(kind, JSON.parse(source)).then((value) => { if (active) { setInfo(value.description); setError(undefined); } }).catch((e) => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, [kind, source]);
  const job = object(card.state.job);
  const frame = cardState({ streaming, id: card.id, status: card.proposal?.status, busy: card.busy, error: card.error, job: job.phase });
  const pending = !card.proposal || card.proposal.status === "pending";
  const codes = Array.isArray(input.subjects) ? input.subjects.filter((v): v is string => typeof v === "string") : [];
  const subjectNames = codes.map((code) => {
    const matches = subjects.filter((s) => s.code.toLowerCase() === code.toLowerCase() || s.code.toLowerCase().startsWith(`${code.toLowerCase()}_`));
    const current = matches.filter((s) => s.is_current);
    const subject = current.length === 1 ? current[0] : matches.length === 1 ? matches[0] : undefined;
    return subject?.name ?? displayCode(code);
  });
  const targets = kind === "Sync" ? `${subjectNames.join(", ")}${input.lectures ? " · including lectures" : ""}` : Object.values(input).filter((value) => value != null).map((value) => Array.isArray(value) ? value.join(", ") : typeof value === "boolean" ? value ? "including lectures" : "" : String(value)).filter(Boolean).join(" · ");
  const actions = frame.failure === "load"
    ? <Button variant="outline" size="sm" onClick={card.retry}>Try again</Button>
    : pending && <><Button variant="ghost" size="sm" disabled={!frame.editable} onClick={() => card.act("discard")}>Discard</Button><Button size="sm" disabled={!frame.editable || !info || !!error} onClick={() => card.act("start")}>{card.busy && <SpinnerGap className="animate-spin" />}{LABELS[kind]}</Button></>;
  if (frame.collapsed) return <CardFrame label="Job proposal" state={frame} icon={Icon} title={LABELS[kind]} meta={<Badge variant="secondary">Discarded</Badge>} />;
  // Pending shows the job's description; once started, JobProgress owns the body and shows any error once.
  return <CardFrame label="Job proposal" state={frame} icon={Icon} actions={actions}
    title={`${LABELS[kind]}${pending ? "?" : ""}`}
    subtitle={<p title={codes.join(", ") || undefined} className="mt-1 text-xs break-words text-muted-foreground">{targets}</p>}
    status={frame.phase === "applying" ? card.busy ? "Starting…" : "Running…" : frame.phase === "created" ? "Complete" : undefined}>
    {pending ? <>
      {!(error || card.error) && <p className="border-t border-border-subtle px-4 py-2 text-[13px] leading-relaxed text-muted-foreground">{info ?? "Loading job details…"}</p>}
      {!!(card.error || error) && <p role="alert" className="border-t border-border-subtle px-4 py-2 text-xs text-destructive">{card.error ?? error}</p>}
    </> : <JobProgress kind={kind} state={card.state} error={card.error} />}
  </CardFrame>;
}
