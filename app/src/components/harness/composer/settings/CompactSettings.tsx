import { memo, useState } from "react";
import { SlidersHorizontal } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ProviderMark } from "@/components/icons/ProviderMark";
import { MODE_LABEL, modeChoices, type PickerProvider, type Provider, type RateWindow, type ThreadUsage } from "@/lib/harness";
import { cn } from "@/lib/utils";
import { MODEL_MENU_FRAME, ModelMenu } from "../model/ModelMenu";
import { modelSummary } from "../model/modelSummary";
import { ModeIcon, ModeNote, PermissionModeChoices } from "../PermissionModeControl";
import { usePermissionMode } from "../usePermissionMode";
import { Ring, UsageDetails, usageSummary } from "../UsageMeter";
import { PageFrame } from "./PageFrame";
import type { SettingsPage } from "./pages";
import { PageHeader, SettingsRow } from "./SettingsRow";

/**
 * The compact composer's settings button and its menu, in place of the full
 * box's model picker, mode control and usage ring. The menu is one popover
 * whose pages change in place (`PageFrame`): the list of settings, each row
 * an icon, the setting, its value and a chevron, opening its page — the
 * model picker's own menu (`ModelMenu`: agents, models, level), the
 * permission mode (a new chat's too, which it starts with), and usage (the
 * nearest plan limit on the row, every figure on the page). The button's
 * title names model and level, so the selection is never hidden. `/model`
 * opens the menu on the model page (`modelOpen`), which is also what hides
 * the composer's own refusal line while the page shows it.
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
  /** The model page is showing (the composer's `/model` sets it). */
  modelOpen: boolean;
  onModelOpenChange: (open: boolean) => void;
  threadId: number | null;
  usage: ThreadUsage | null;
  rateLimits: RateWindow[];
}) {
  const [menuOpen, setMenuOpen] = useState(false);
  /** The page showing other than the model's, which `modelOpen` owns. */
  const [other, setOther] = useState<Exclude<SettingsPage, "model">>("root");
  /** The page the list was reached back from, so its row takes focus. */
  const [from, setFrom] = useState<SettingsPage | null>(null);
  /** Moved between pages since opening: later pages slide in. */
  const [moved, setMoved] = useState(false);
  const page: SettingsPage = modelOpen ? "model" : other;

  const summary = modelSummary({ providers, provider, model, reasoning, switchFrom });
  const permission = usePermissionMode(threadId, provider);
  const meter = usageSummary(usage, rateLimits);
  const { mode } = permission;

  const go = (next: SettingsPage) => {
    if (next === "root") setFrom(page);
    if (page === "permission") permission.clear();
    setMoved(true);
    setMenuOpen(true);
    onModelOpenChange(next === "model");
    setOther(next === "model" ? "root" : next);
  };
  const back = () => go("root");
  const close = () => {
    setMenuOpen(false);
    onModelOpenChange(false);
    setOther("root");
    setFrom(null);
    setMoved(false);
    permission.clear();
  };

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
        className="group/settings w-auto overflow-hidden p-0"
        // Each page focuses its own control (`PageFrame`).
        onOpenAutoFocus={(e) => e.preventDefault()}
        onEscapeKeyDown={(e) => {
          if (page === "root") return;
          e.preventDefault();
          back();
        }}
      >
        <PageFrame page={page} from={from} slide={moved} onBack={back}>
          {page === "root" ? (
            <SettingsList
              model={{ label: summary.label ?? "Choose a model", level: summary.levelText, title: summary.title }}
              provider={provider}
              mode={mode}
              usage={meter}
              onOpen={go}
            />
          ) : page === "model" ? (
            <div className={MODEL_MENU_FRAME}>
              <PageHeader title="Model" onBack={back} />
              <ModelMenu
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
            </div>
          ) : page === "permission" ? (
            <div className="w-72">
              <PageHeader
                title="Permission mode"
                onBack={back}
                trailing={<ModeNote note={modeChoices(permission.caps).note} />}
              />
              <PermissionModeChoices
                caps={permission.caps}
                mode={mode}
                busy={permission.busy}
                refused={permission.refused}
                onPick={(next) => permission.pick(next, close)}
              />
            </div>
          ) : (
            <div className="w-64">
              <PageHeader title="Usage" onBack={back} />
              <div className="p-3">
                <UsageDetails summary={meter} rateLimits={rateLimits} />
              </div>
            </div>
          )}
        </PageFrame>
      </PopoverContent>
    </Popover>
  );
});

/**
 * The settings list, the menu's first page: model and level, the permission
 * mode, and usage when there is any, its value the plan window nearest its
 * limit (`usageSummary`'s `headline`). A new setting is one more row.
 */
export function SettingsList({
  model,
  provider,
  mode,
  usage,
  onOpen,
  className,
}: {
  model: { label: string; level: string | null; title: string };
  provider: Provider;
  mode: ReturnType<typeof usePermissionMode>["mode"];
  usage: ReturnType<typeof usageSummary>;
  onOpen: (page: Exclude<SettingsPage, "root">) => void;
  className?: string;
}) {
  return (
    <div className={cn("flex w-64 flex-col p-1", className)}>
      <SettingsRow
        page="model"
        icon={<ProviderMark provider={provider} className="size-3.5" />}
        label={model.label}
        value={model.level}
        title={model.title}
        onOpen={onOpen}
      />
      <SettingsRow
        page="permission"
        icon={<ModeIcon mode={mode} />}
        label="Permission mode"
        value={MODE_LABEL[mode]}
        title={`Permission mode: ${MODE_LABEL[mode]}`}
        onOpen={onOpen}
      />
      {!usage.empty && (
        <SettingsRow
          page="usage"
          icon={<Ring percent={usage.percent} />}
          label="Usage"
          value={usage.headline}
          title={usage.ringLabel ?? undefined}
          onOpen={onOpen}
        />
      )}
    </div>
  );
}
