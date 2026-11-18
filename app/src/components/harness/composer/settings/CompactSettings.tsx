import { memo, useState } from "react";
import { SlidersHorizontal } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ProviderMark } from "@/components/icons/ProviderMark";
import { MODE_LABEL, type PickerProvider, type Provider, type RateWindow, type ThreadUsage } from "@/lib/harness";
import { ModelMenuContent } from "../model/ModelMenu";
import { modelSummary } from "../model/modelSummary";
import { focusPickedMode, ModeIcon, PermissionModeMenu } from "../PermissionModeControl";
import { usePermissionMode } from "../usePermissionMode";
import { Ring, UsageDetails, usageSummary } from "../UsageMeter";
import { SettingsSubmenu, SUBMENU_PLACEMENT } from "./SettingsSubmenu";

/** The submenu open beside the settings menu; one at a time. */
type Sub = "model" | "permission" | "usage";

/**
 * The compact composer's settings button and its menu, in place of the full
 * box's model picker, mode control and usage ring. One row per setting
 * (`SettingsSubmenu`): the model and level, opening the model picker's own
 * menu; the permission mode, for an open thread; usage, when there is any.
 * The button's title names model and level, so the selection is never
 * hidden. `/model` opens the menu on its model row (`modelOpen`), which is
 * also what hides the composer's own refusal line while the picker shows it.
 */
export const CompactSettings = memo(function CompactSettings({
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
  modelOpen,
  onModelOpenChange,
  threadId,
  usage,
  rateLimits,
}: {
  providers: PickerProvider[];
  provider: Provider;
  locked?: (provider: Provider) => boolean;
  refused?: string | null;
  switchFrom?: Provider | null;
  model: string | null;
  reasoning: string | null;
  onProvider: (p: Provider) => void;
  onModel: (m: string | null) => void;
  onReasoning: (level: string | null) => void;
  /** The model submenu is open (the composer's `/model` sets it). */
  modelOpen: boolean;
  onModelOpenChange: (open: boolean) => void;
  threadId: number | null;
  usage: ThreadUsage | null;
  rateLimits: RateWindow[];
}) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [other, setOther] = useState<Exclude<Sub, "model"> | null>(null);
  const sub: Sub | null = modelOpen ? "model" : other;
  const setSub = (next: Sub | null) => {
    onModelOpenChange(next === "model");
    setOther(next === "model" ? null : next);
  };
  const close = () => {
    setMenuOpen(false);
    setSub(null);
  };
  const toggle = (row: Sub) => (open: boolean) => setSub(open ? row : null);

  const summary = modelSummary({ providers, provider, model, reasoning, switchFrom });
  const permission = usePermissionMode(threadId, provider);
  const meter = usageSummary(usage, rateLimits);
  const mode = permission.mode;

  return (
    <Popover open={menuOpen || modelOpen} onOpenChange={(open) => (open ? setMenuOpen(true) : close())}>
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label={`Settings — ${summary.title}`}
          title={summary.title}
          className="shrink-0 text-muted-foreground hover:text-foreground"
        >
          <SlidersHorizontal />
        </Button>
      </PopoverTrigger>
      <PopoverContent
        side="top"
        align="end"
        sideOffset={6}
        collisionPadding={8}
        className="flex w-60 flex-col p-1"
        // Opened by `/model`, the submenu takes focus; the menu must not pull it back.
        onOpenAutoFocus={(e) => {
          if (sub) e.preventDefault();
        }}
      >
        <SettingsSubmenu
          open={sub === "model"}
          onOpenChange={toggle("model")}
          icon={<ProviderMark provider={provider} className="size-3.5" />}
          label={summary.label ?? "Choose a model"}
          value={summary.levelText}
          title={summary.title}
        >
          <ModelMenuContent
            {...SUBMENU_PLACEMENT}
            providers={providers}
            provider={provider}
            locked={locked}
            refused={refused}
            model={model}
            reasoning={reasoning}
            onProvider={onProvider}
            onModel={onModel}
            onReasoning={onReasoning}
            onClose={close}
          />
        </SettingsSubmenu>

        {mode != null && (
          <SettingsSubmenu
            open={sub === "permission"}
            onOpenChange={(open) => {
              toggle("permission")(open);
              if (!open) permission.clear();
            }}
            icon={<ModeIcon mode={mode} />}
            label={MODE_LABEL[mode]}
            title={`Permission mode: ${MODE_LABEL[mode]}`}
          >
            <PopoverContent
              {...SUBMENU_PLACEMENT}
              className="w-72 max-w-[calc(100vw-16px)] p-0"
              onOpenAutoFocus={focusPickedMode}
            >
              <PermissionModeMenu
                caps={permission.caps}
                mode={mode}
                busy={permission.busy}
                refused={permission.refused}
                onPick={(next) => permission.pick(next, close)}
              />
            </PopoverContent>
          </SettingsSubmenu>
        )}

        {!meter.empty && (
          <SettingsSubmenu
            open={sub === "usage"}
            onOpenChange={toggle("usage")}
            icon={<Ring percent={meter.contextPercent} />}
            label="Usage"
            value={meter.parts.join(" · ")}
            title={meter.parts.join(" · ")}
          >
            <PopoverContent {...SUBMENU_PLACEMENT} className="w-64 p-3">
              <UsageDetails summary={meter} rateLimits={rateLimits} />
            </PopoverContent>
          </SettingsSubmenu>
        )}
      </PopoverContent>
    </Popover>
  );
});
