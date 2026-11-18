import { memo, useState, type ReactNode } from "react";
import { CaretDown } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverTrigger } from "@/components/ui/popover";
import { ProviderMark } from "@/components/icons/ProviderMark";
import type { PickerProvider, Provider } from "@/lib/harness";
import { cn } from "@/lib/utils";
import { ModelMenuContent } from "./model/ModelMenu";
import { modelSummary } from "./model/modelSummary";

/** `locked` for a picker whose chat keeps its agent whatever the bridges can do. */
export const LOCK_OTHERS = () => true;

export type { PickerProvider } from "@/lib/harness";

/**
 * The composer's model-and-reasoning switcher (after bb's
 * `ModelReasoningPicker`): a trigger naming the model and level, opening
 * `ModelMenuContent`. Stable selection props keep composer draft keystrokes
 * outside this catalogue's memo boundary.
 */
export const ModelPicker = memo(function ModelPicker({
  providers,
  provider,
  locked,
  refused,
  switchFrom,
  model,
  reasoning,
  onProvider,
  onModel,
  onReasoning,
  align = "start",
  open: openProp,
  onOpenChange,
  emptyLabel = "Choose a model",
  emptyIcon,
  keepEmptyTrigger = false,
  className,
}: {
  providers: PickerProvider[];
  provider: Provider;
  /** The agents this chat cannot move to (`switchLocked`); a stable function. */
  locked?: (provider: Provider) => boolean;
  /** Rust's reason for refusing the last pick, shown above the models. */
  refused?: string | null;
  /** The agent the chat moves away from with its next message, when the
   *  pick names another (`pendingSwitch`). */
  switchFrom?: Provider | null;
  model: string | null;
  reasoning: string | null;
  onProvider: (p: Provider) => void;
  onModel: (m: string | null) => void;
  onReasoning: (level: string | null) => void;
  /** Which trigger edge the menu lines up with; `end` for a trigger at a page's right edge. */
  align?: "start" | "end";
  /** Given, the caller owns whether the menu is open (the composer's `/model`
   *  opens it); `onOpenChange` then hears every change. */
  open?: boolean;
  /** Told whenever the menu opens or closes. */
  onOpenChange?: (open: boolean) => void;
  /** The trigger's text while no model is set. */
  emptyLabel?: string;
  /** Replaces the agent's mark while no model is set, for an "add" trigger;
   *  null shows none. */
  emptyIcon?: ReactNode;
  /** The trigger keeps its empty look with a model set: an "add" menu whose
   *  pick lands in a row of its own. */
  keepEmptyTrigger?: boolean;
  className?: string;
}) {
  const [openState, setOpenState] = useState(false);
  const open = openProp ?? openState;
  const setOpen = (next: boolean) => {
    setOpenState(next);
    onOpenChange?.(next);
  };
  const { label, levelText, title } = modelSummary({ providers, provider, model, reasoning, switchFrom });

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          aria-label="Model and reasoning"
          title={title}
          className={cn(
            "h-6 max-w-[280px] gap-1.5 px-1.5 text-[11px] font-normal text-muted-foreground hover:text-foreground",
            className,
          )}
        >
          {(label == null || keepEmptyTrigger) && emptyIcon !== undefined ? (
            emptyIcon
          ) : (
            <ProviderMark provider={provider} className="size-3.5 shrink-0" />
          )}
          <span className={cn("min-w-0 truncate", label != null && !keepEmptyTrigger && "text-foreground")}>
            {keepEmptyTrigger ? emptyLabel : (label ?? emptyLabel)}
          </span>
          {levelText && !keepEmptyTrigger ? <span className="shrink-0 text-muted-foreground">{levelText}</span> : null}
          <CaretDown className="size-3 shrink-0 text-muted-foreground" />
        </Button>
      </PopoverTrigger>
      <ModelMenuContent
        align={align}
        providers={providers}
        provider={provider}
        locked={locked}
        refused={refused}
        model={model}
        reasoning={reasoning}
        onProvider={onProvider}
        onModel={onModel}
        onReasoning={onReasoning}
        onClose={() => setOpen(false)}
      />
    </Popover>
  );
});
