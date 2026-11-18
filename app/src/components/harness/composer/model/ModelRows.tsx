import type { ReactNode } from "react";
import { Check, LockSimple, Star } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { ProviderMark } from "@/components/icons/ProviderMark";
import { providerLabel, SWITCH_LOCKED, type PickerProvider, type Provider } from "@/lib/harness";
import type { ModelStar } from "@/stores/chat/modelStars";
import { settingsPage, type SettingsPageId } from "@/lib/search/settings";
import { cn } from "@/lib/utils";

/** Shown in place of a locked agent's catalogue: this chat cannot move to it. */
export function SwitchLocked({ label }: { label: string }) {
  return (
    <div className="flex items-start gap-2 px-2 py-3 text-xs text-muted-foreground">
      <LockSimple className="mt-0.5 size-3.5 shrink-0" aria-hidden />
      <span>
        {label} — {SWITCH_LOCKED}
      </span>
    </div>
  );
}

/** Shown in place of the catalogue when the CLI wasn't found. Navigates via
 *  `navigateActive`, since a portalled popover has no router to `useNavigate`. */
export function NotInstalled({ label, onSettings }: { label: string; onSettings: () => void }) {
  return (
    <div className="px-2 py-3">
      <div className="text-xs text-foreground">{label} is not installed</div>
      <p className="mt-1 text-[11px] leading-relaxed text-muted-foreground">
        Chat runs the CLI from this machine, so there is nothing to send to until it is
        on your PATH.
      </p>
      <Button variant="outline" size="xs" className="mt-2.5" onClick={onSettings}>
        Open Settings → Agents
      </Button>
    </div>
  );
}

/** An empty catalogue with the provider's own reason and a way into the
 *  Settings page that fixes it. */
export function EmptyNote({
  note,
  page,
  onSettings,
}: {
  note: string;
  page: SettingsPageId;
  onSettings: (page: SettingsPageId) => void;
}) {
  return (
    <div className="px-2 py-3">
      <p className="text-[11px] leading-relaxed text-muted-foreground">{note}</p>
      <Button variant="outline" size="xs" className="mt-2.5" onClick={() => onSettings(page)}>
        Open Settings → {settingsPage(page).label}
      </Button>
    </div>
  );
}

/** The starred models across agents, in starring order. A star on an agent
 *  this chat cannot move to, or whose CLI is missing, is dimmed and says why. */
export function StarredList({
  stars,
  providers,
  provider,
  model,
  pending,
  locked,
  onPick,
  onUnstar,
}: {
  stars: ModelStar[];
  providers: PickerProvider[];
  provider: Provider;
  model: string | null;
  pending: { provider: Provider; model: string } | null;
  locked?: (provider: Provider) => boolean;
  onPick: (star: ModelStar) => void;
  onUnstar: (star: ModelStar) => void;
}) {
  if (stars.length === 0) {
    return (
      <div className="px-2 py-3 text-[11px] leading-relaxed text-muted-foreground">
        Star a model on an agent's tab to keep it here.
      </div>
    );
  }
  return (
    <>
      <SectionLabel>Starred</SectionLabel>
      {stars.map((star) => {
        const p = providers.find((x) => x.id === star.provider);
        const gone = !p || p.health === "missing";
        const shut = star.provider !== provider && !!locked?.(star.provider);
        const current = pending
          ? pending.provider === star.provider && pending.model === star.model
          : star.provider === provider && star.model === model;
        const agent = p?.label ?? providerLabel(star.provider);
        return (
          <ModelRow
            key={`${star.provider}:${star.model}`}
            label={p?.models.find((m) => m.id === star.model)?.label ?? star.label}
            title={[agent, star.model, shut && SWITCH_LOCKED, gone && "not installed"].filter(Boolean).join(" · ")}
            mark={<ProviderMark provider={star.provider} className="size-3.5 shrink-0" />}
            dim={gone || shut}
            selected={current}
            starred
            onStar={() => onUnstar(star)}
            onClick={() => onPick(star)}
          />
        );
      })}
    </>
  );
}

export function ModelRow({
  label,
  description,
  title,
  mark,
  dim = false,
  selected,
  starred,
  onStar,
  onClick,
}: {
  label: string;
  description?: string;
  title?: string;
  /** The agent's mark, in the Starred tab where rows mix agents. */
  mark?: ReactNode;
  dim?: boolean;
  selected: boolean;
  starred: boolean;
  onStar: () => void;
  onClick: () => void;
}) {
  return (
    <div className="group/row flex w-full items-center rounded-md transition-colors hover:bg-accent has-[:focus-visible]:bg-accent">
      <button
        type="button"
        title={title ?? description}
        aria-current={selected || undefined}
        onClick={onClick}
        className={cn(
          "flex min-w-0 flex-1 cursor-default select-none items-center gap-2 py-1.5 pl-2 text-left text-xs text-foreground outline-none",
          dim && "opacity-50",
        )}
      >
        {mark}
        <span className="min-w-0 truncate">{label}</span>
        <Check
          className={cn("ml-auto size-3.5 shrink-0 text-muted-foreground", selected ? "opacity-100" : "opacity-0")}
        />
      </button>
      <button
        type="button"
        aria-label={starred ? `Unstar ${label}` : `Star ${label}`}
        title={starred ? "Unstar" : "Star"}
        aria-pressed={starred}
        onMouseDown={(e) => e.preventDefault()}
        onClick={onStar}
        className={cn(
          "flex size-7 shrink-0 cursor-default items-center justify-center rounded-md outline-none transition-opacity focus-visible:opacity-100",
          starred
            ? "text-warning"
            : "text-muted-foreground opacity-0 hover:text-foreground group-hover/row:opacity-100",
        )}
      >
        <Star className="size-3.5" weight={starred ? "fill" : "regular"} />
      </button>
    </div>
  );
}

export function SectionLabel({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div
      className={cn(
        "sticky top-0 z-10 bg-popover px-2 pb-1 pt-2 text-[11px] font-medium text-muted-foreground",
        className,
      )}
    >
      {children}
    </div>
  );
}
