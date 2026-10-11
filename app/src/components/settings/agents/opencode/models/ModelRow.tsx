import { Brain, FilePdf, Image as ImageIcon, Paperclip, VideoCamera, Waveform, type Icon } from "@phosphor-icons/react";
import type { HarnessModel, ModelFacts } from "@/lib/harness";
import { cn } from "@/lib/utils";
import { Checkbox } from "@/components/ui/checkbox";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { blockedBecause } from "../useOpencode";
import { COLS } from "./constants";
import { fmtPrice, fmtRelease, fmtTokens } from "./format";

export function ModelRow({
  model,
  provider,
  hidden,
  onHidden,
}: {
  model: HarnessModel;
  provider: string;
  hidden: boolean;
  onHidden: (hidden: boolean) => void;
}) {
  // A model the gate never offers can't be ticked; hovering its name says why.
  const why = blockedBecause(model);
  const blocked = why !== null;
  const f = model.facts;

  return (
    <div className={cn(COLS, "py-2 transition-colors hover:bg-surface/60")}>
      <Checkbox
        aria-label={`Offer ${model.label}`}
        checked={!blocked && !hidden}
        disabled={blocked}
        onCheckedChange={(v) => onHidden(v !== true)}
      />

      {why ? (
        <Tooltip>
          <TooltipTrigger asChild>
            <div className="min-w-0">
              <Name model={model} blocked />
            </div>
          </TooltipTrigger>
          <TooltipContent>{why}</TooltipContent>
        </Tooltip>
      ) : (
        <div className="min-w-0">
          <Name model={model} blocked={false} />
        </div>
      )}

      <span className="truncate text-xs text-muted-foreground">{provider}</span>

      <Num>{fmtPrice(f?.cost?.input)}</Num>
      <Num>{fmtPrice(f?.cost?.output)}</Num>
      <Num>{fmtTokens(f?.context)}</Num>
      <Num>{fmtTokens(f?.maxOutput)}</Num>
      <Capabilities facts={f} />
      <Num>{fmtRelease(f?.releaseDate)}</Num>
    </div>
  );
}

function Name({ model, blocked }: { model: HarnessModel; blocked: boolean }) {
  return (
    <>
      <div className={cn("truncate text-xs", blocked ? "text-muted-foreground" : "text-foreground")}>
        {model.label}
      </div>
      <div className="truncate text-[11px] text-muted-foreground">{model.id}</div>
    </>
  );
}

function Num({ children }: { children: string }) {
  return (
    <span
      className={cn(
        "justify-self-end text-xs tabular-nums",
        children === "—" ? "text-muted-foreground/60" : "text-foreground",
      )}
    >
      {children}
    </span>
  );
}

const INPUTS: ReadonlyArray<{ key: string; label: string; icon: Icon }> = [
  { key: "image", label: "Takes images", icon: ImageIcon },
  { key: "pdf", label: "Takes PDFs", icon: FilePdf },
  { key: "audio", label: "Takes audio", icon: Waveform },
  { key: "video", label: "Takes video", icon: VideoCamera },
];

/** Icons drawn inline; past this the rest fold into a "+N" that opens the full list. */
const SHOWN = 3;

/** What the model can do beyond text; tool calling is a block reason, not an icon. */
function Capabilities({ facts }: { facts: ModelFacts | undefined }) {
  if (!facts) return <span className="text-xs text-muted-foreground/60">—</span>;
  const marks: Array<{ label: string; icon: Icon }> = [];
  if (facts.reasoning) marks.push({ label: "Reasons", icon: Brain });
  for (const input of INPUTS) if (facts.inputs.includes(input.key)) marks.push(input);
  if (facts.attachment) marks.push({ label: "Takes attachments", icon: Paperclip });
  if (marks.length === 0) return <span className="text-xs text-muted-foreground/60">—</span>;
  const rest = marks.length - SHOWN;
  return (
    <div className="flex items-center gap-1.5 text-muted-foreground">
      {marks.slice(0, SHOWN).map(({ label, icon: Mark }) => (
        <Tooltip key={label}>
          <TooltipTrigger asChild>
            <span aria-label={label} className="flex">
              <Mark size={13} />
            </span>
          </TooltipTrigger>
          <TooltipContent>{label}</TooltipContent>
        </Tooltip>
      ))}
      {rest > 0 && (
        <Popover>
          <PopoverTrigger asChild>
            <button
              type="button"
              aria-label={`Show all ${marks.length} capabilities`}
              className="h-4.5 cursor-pointer rounded-full bg-surface px-1.5 text-[10.5px] text-muted-foreground tabular-nums transition-colors hover:bg-accent hover:text-foreground"
            >
              +{rest}
            </button>
          </PopoverTrigger>
          <PopoverContent align="end" className="w-44 p-1.5">
            {marks.map(({ label, icon: Mark }) => (
              <div
                key={label}
                className="flex items-center gap-2 px-2 py-1 text-[12.5px] text-foreground"
              >
                <Mark size={13} className="shrink-0 text-muted-foreground" />
                {label}
              </div>
            ))}
          </PopoverContent>
        </Popover>
      )}
    </div>
  );
}
