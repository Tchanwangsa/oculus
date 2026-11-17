import { useContext, useEffect, useState, type ReactNode } from "react";
import { StreamingPending } from "../Streaming";
import { FilePdf, FileVideo, ListNumbers, SpinnerGap, Waveform } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { proposalJobInfo } from "@/lib/harness/proposals";
import { JobProgress, useJobProgress } from "./JobProgress.tsx";
import { useProposal } from "./useProposal";
import { CardFrame, CardSkeleton, useCardStreaming } from "./CardFrame";
import { cardState } from "./cardState";
import { kindIcon, kindNoun } from "./kinds";
import { CHIP, SubjectLabel } from "./SubjectLabel";
import { type Row } from "./model";
import { continueLabel, followUp, followUpNote } from "./followUp";

const LABELS: Record<string, string> = { Sync: "Sync", Reparse: "Re-parse", Transcribe: "Transcribe", LectureChapters: "Find chapters", LectureEnd: "Find the end", Download: "Download" };

const strings = (value: unknown) => Array.isArray(value) ? value.filter((v): v is string => typeof v === "string") : [];
const base = (path: string) => path.split("/").pop() ?? path;

/** The job as a question: what it acts on, in words a student uses. */
function question(kind: string, input: Row): string {
  const count = (n: number, one: string, many: string) => n === 1 ? one : `${n} ${many}`;
  if (kind === "Sync") return `Sync ${count(strings(input.subjects).length, "this subject", "subjects")}?`;
  if (kind === "Reparse") return `Re-parse ${count(strings(input.files).length, "this file", "files")}?`;
  if (kind === "Transcribe") return "Transcribe this recording?";
  if (kind === "LectureChapters") return "Find this lecture’s chapters?";
  if (kind === "LectureEnd") return "Find where this lecture ends?";
  const item = String(input.item ?? "");
  return item === "voices" ? "Download the starter voices?" : item === "aligner" ? "Download the word aligner?" : `Download the ${item} voice?`;
}

/** What the job acts on, as the app's own chips: subjects with their icons, files by name. */
function targets(kind: string, input: Row): ReactNode {
  const chip = (key: string, icon: ReactNode, text: string, title?: string) => <span key={key} title={title} className={CHIP}>{icon}<span className="min-w-0 truncate">{text}</span></span>;
  if (kind === "Sync") return <>{strings(input.subjects).map((code) => <SubjectLabel key={code} code={code} />)}{input.lectures === true && chip("lectures", null, "With lectures")}</>;
  if (kind === "Reparse") return strings(input.files).map((file) => chip(file, <FilePdf size={11} className="shrink-0" />, base(file), file));
  if (kind === "Transcribe" && typeof input.file === "string") return chip("file", <FileVideo size={11} className="shrink-0" />, base(input.file), input.file);
  if ((kind === "LectureChapters" || kind === "LectureEnd") && input.lecture != null) return chip("lecture", kind === "LectureChapters" ? <ListNumbers size={11} className="shrink-0" /> : <Waveform size={11} className="shrink-0" />, `Lecture ${String(input.lecture)}`);
  return null;
}

export function JobCard({ kind, input }: { kind: string; input: Row }) {
  const streaming = useCardStreaming();
  const pending = useContext(StreamingPending);
  if (streaming && pending) return <CardSkeleton icon={kindIcon(kind)} title={`Drawing ${kindNoun(kind)}…`} />;
  return <JobCardContent kind={kind} input={input} />;
}

function JobCardContent({ kind, input }: { kind: string; input: Row }) {
  const card = useProposal(kind, input);
  const streaming = useCardStreaming();
  const [info, setInfo] = useState<string>();
  const [infoError, setInfoError] = useState<string>();
  const source = JSON.stringify(input);
  useEffect(() => {
    if (streaming) return;
    let active = true;
    setInfo(undefined);
    setInfoError(undefined);
    void proposalJobInfo(kind, JSON.parse(source)).then((value) => { if (active) { setInfo(value.description); setInfoError(undefined); } }).catch((e) => { if (active) setInfoError(String(e)); });
    return () => { active = false; };
  }, [kind, source, streaming]);
  const run = useJobProgress(kind, card.state);
  const frame = cardState({ streaming, id: card.id, status: card.proposal?.status, busy: card.busy, error: card.error, job: run.job.phase });
  const pending = !card.proposal || card.proposal.status === "pending";
  const follow = followUp(card.state);
  const note = followUpNote(follow, frame);
  const ask = question(kind, input);
  const title = pending ? ask : ask.replace(/\?$/, "");
  const chips = targets(kind, input);
  const subtitle = chips && <div className="mt-1 flex min-w-0 flex-wrap items-center gap-1">{chips}</div>;
  // A refused decision or the details' error sits under the card; the job's
  // own leads its body, above how long it ran.
  const error = card.error ?? (pending ? infoError : undefined);
  const jobError = pending || card.error ? undefined : run.job.phase === "error" ? String(run.job.error ?? "The job couldn’t finish.") : run.error;
  const actions = pending && <><Button variant="ghost" size="sm" disabled={!frame.editable} onClick={() => card.act("discard")}>{continueLabel("Discard", follow)}</Button><Button size="sm" disabled={!frame.editable || !info || !!infoError} onClick={() => card.act("start")}>{card.busy && <SpinnerGap className="animate-spin" />}{continueLabel(LABELS[kind], follow)}</Button></>;
  if (frame.collapsed) return <CardFrame label="Job proposal" state={frame} icon={kindIcon(kind)} title={title} note={note} />;
  return <CardFrame label="Job proposal" state={frame} icon={kindIcon(kind)} title={title} subtitle={subtitle} actions={actions} note={note} error={error} onRetry={card.retry}
    status={frame.phase === "applying" ? card.busy ? "Starting…" : "Running" : frame.phase === "created" ? "Complete" : undefined}>
    {/* Pending shows what the job costs; while the details load, a line of the same height. */}
    {pending ? !infoError && <div className="px-4 py-2.5 text-[13px] leading-5 text-muted-foreground">{info ?? <div aria-label="Loading job details" className="block-shimmer my-1 h-3 w-3/4 rounded-full" />}</div>
      : <JobProgress run={run} error={jobError} />}
  </CardFrame>;
}
