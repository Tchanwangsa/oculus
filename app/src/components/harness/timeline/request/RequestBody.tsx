import { Globe, Warning } from "@phosphor-icons/react";
import { CodeText } from "@/components/markdown/MdComponents";
import { changeKindLabel, type RequestDetail } from "@/lib/harness";
import { DiffBlock } from "./DiffBlock";
import { FoldBlock } from "./FoldBlock";

/** What a request asks, per action: the whole command with its folder and,
 *  only where Rust says so, that it runs outside the sandbox; each file with
 *  its diff; the access asked for; what another tool call will do, with its
 *  arguments; then the agent's reason. */
export function RequestBody({ detail }: { detail: RequestDetail }) {
  return (
    <div className="flex min-w-0 flex-col gap-1.5" data-selectable>
      {detail.command != null && (
        <div className="min-w-0">
          <CodeText className="rounded-md bg-surface px-2.5 py-1.5 text-[11.5px] text-foreground/85 break-all">
            {detail.command}
          </CodeText>
          {detail.cwd && (
            <p className="mt-0.5 break-all text-[11px] leading-relaxed text-muted-foreground">in {detail.cwd}</p>
          )}
        </div>
      )}
      {detail.network && (
        <p className="flex items-center gap-1 text-[11px] text-muted-foreground">
          <Globe size={12} className="shrink-0" aria-hidden />
          Needs the network
        </p>
      )}
      {detail.outsideSandbox && (
        <p className="flex items-center gap-1 text-[11px] font-medium text-warning">
          <Warning size={12} weight="bold" className="shrink-0" aria-hidden />
          Runs outside the sandbox
        </p>
      )}
      {detail.changes.map((c) => (
        <div key={c.path} className="min-w-0">
          <p className="flex min-w-0 items-baseline gap-1.5 text-[11.5px]">
            <span className="min-w-0 break-all font-medium text-foreground/85">{c.path}</span>
            <span className="shrink-0 text-[11px] text-muted-foreground">{changeKindLabel(c)}</span>
          </p>
          {c.diff && <DiffBlock diff={c.diff} />}
        </div>
      ))}
      {detail.access.length > 0 && (
        <ul className="flex flex-col gap-0.5 text-[11.5px] text-foreground/85">
          {detail.access.map((line) => (
            <li key={line} className="break-all">{line}</li>
          ))}
        </ul>
      )}
      {detail.tool && (
        <div className="min-w-0">
          <p className="break-all text-[11.5px] font-medium text-foreground/85">{detail.tool.line}</p>
          {detail.tool.note && (
            <p className="break-words text-[11px] leading-relaxed text-muted-foreground">{detail.tool.note}</p>
          )}
          {detail.tool.args && (
            <FoldBlock text={detail.tool.args}>
              <CodeText className="px-2.5 text-[11.5px] text-foreground/85">{detail.tool.args}</CodeText>
            </FoldBlock>
          )}
        </div>
      )}
      {detail.reason && (
        <p className="break-words text-[11.5px] leading-relaxed text-muted-foreground">“{detail.reason}”</p>
      )}
    </div>
  );
}
