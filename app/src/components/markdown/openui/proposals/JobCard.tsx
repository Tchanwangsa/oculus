import { useEffect, useState } from "react";
import { Check, DownloadSimple, SpinnerGap } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { proposalJobInfo } from "@/lib/harness/proposals";
import { useProposal } from "./useProposal";
import { object, type Row } from "./model";

const LABELS: Record<string, string> = { Sync: "Sync", Reparse: "Re-parse", Transcribe: "Transcribe", LectureChapters: "Find lecture chapters", LectureEnd: "Find the lecture end", Download: "Download" };

export function JobCard({ kind, input }: { kind: string; input: Row }) {
  const card = useProposal(kind, input);
  const [info, setInfo] = useState<string>();
  const [error, setError] = useState<string>();
  const source = JSON.stringify(input);
  useEffect(() => {
    let active = true;
    void proposalJobInfo(kind, JSON.parse(source)).then((value) => { if (active) { setInfo(value.description); setError(undefined); } }).catch((e) => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, [kind, source]);
  const job = object(card.state.job);
  const pending = !card.proposal || card.proposal.status === "pending";
  const running = job.phase === "running";
  const targets = Object.values(input).filter((value) => value != null).map((value) => Array.isArray(value) ? value.join(", ") : typeof value === "boolean" ? value ? "including lectures" : "" : String(value)).filter(Boolean).join(" · ");
  if (card.proposal?.status === "discarded") return <p className="my-3 rounded-xl border border-border px-4 py-3 text-[13px] text-muted-foreground">Discarded · {LABELS[kind]}</p>;
  return <section aria-label="Job proposal" className="my-3 overflow-hidden rounded-xl border border-border bg-card">
    <header className="flex items-start gap-2.5 px-4 py-3"><DownloadSimple size={18} className="mt-0.5 shrink-0 text-brand" /><div className="min-w-0"><h4 className="text-[13px] font-semibold">{LABELS[kind]}{pending ? "?" : ""}</h4><p className="mt-1 text-xs break-words text-muted-foreground">{targets}</p></div></header>
    <p className="border-t border-border-subtle px-4 py-2 text-[13px] leading-relaxed text-muted-foreground">{info ?? "Loading job details…"}</p>
    {!!(card.error || error || job.error) && <p role="alert" className="px-4 pb-2 text-xs text-destructive">{card.error ?? error ?? String(job.error)}</p>}
    <footer className="flex flex-wrap items-center justify-end gap-2 border-t border-border-subtle bg-surface/40 px-4 py-3">
      {pending ? <><Button variant="ghost" size="sm" disabled={!card.proposal || card.busy} onClick={() => card.act("discard")}>Discard</Button><Button size="sm" disabled={!card.proposal || card.busy || !info || !!error} onClick={() => card.act("start")}>{card.busy && <SpinnerGap className="animate-spin" />}{LABELS[kind]}</Button></> : <span role="status" className="mr-auto inline-flex items-center gap-2 text-[13px] text-muted-foreground">{running ? <SpinnerGap size={14} className="animate-spin" /> : job.phase === "complete" ? <Check size={14} className="text-success" /> : null}{running ? "Running…" : job.phase === "complete" ? "Complete" : "Failed"}</span>}
    </footer>
  </section>;
}
