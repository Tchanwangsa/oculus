import { Check, GlobeSimple } from "@phosphor-icons/react";

import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { Section } from "@/components/studio/themes/inspector/Fields";
import { displayCode } from "@/lib/format/format";
import type { ComponentLevel } from "@/lib/studio/components";
import type { ThemeDefaults, ThemeLevel } from "@/lib/studio/themes";
import { cn } from "@/lib/utils";

/**
 * "Default for": the libraries this theme is the default of. Global, and the
 * browsed subject when there is one; a press makes it the library's default
 * (its `library.json`) or, when it already is, clears it. A subject's own
 * theme cannot be the global default, which only reaches global and built-in
 * themes.
 */
export function DefaultScope({
  name,
  level,
  subject,
  defaults,
  error,
  onSet,
}: {
  name: string;
  level: ThemeLevel;
  subject: string | null;
  defaults: ThemeDefaults | null;
  error: string | null;
  onSet: (level: ComponentLevel, name: string | null) => void;
}) {
  const globalOn = defaults?.global === name;
  const subjectOn = defaults?.subject === name;
  return (
    <Section
      title="Default for"
      note="A new video takes its subject's default, else the global one. Any video can still pick another theme from its style chip."
    >
      <div className="flex flex-wrap gap-1.5">
        <ScopeChip
          on={globalOn}
          disabled={level === "subject" || defaults == null}
          title={
            level === "subject"
              ? "A subject's own theme can only be that subject's default"
              : globalOn
                ? "Stop being the global default"
                : defaults?.global
                  ? `Now ${defaults.global}`
                  : "Make this the global default"
          }
          onClick={() => onSet("global", globalOn ? null : name)}
        >
          <GlobeSimple size={12} />
          Global
        </ScopeChip>
        {subject && (
          <ScopeChip
            on={subjectOn}
            disabled={defaults == null}
            title={subjectOn ? "Stop being this subject's default" : defaults?.subject ? `Now ${defaults.subject}` : `Make this ${displayCode(subject)}'s default`}
            onClick={() => onSet("subject", subjectOn ? null : name)}
          >
            <SubjectIcon code={subject} size={12} />
            {displayCode(subject)}
          </ScopeChip>
        )}
      </div>
      {defaults && !globalOn && !subjectOn && (
        <p className="text-[11.5px] text-muted-foreground">
          New videos here use <span className="text-foreground">{defaults.effective}</span>.
        </p>
      )}
      {error && (
        <p data-selectable className="text-[12px] leading-snug text-destructive">
          {error}
        </p>
      )}
    </Section>
  );
}

function ScopeChip({
  on,
  disabled,
  title,
  onClick,
  children,
}: {
  on: boolean;
  disabled: boolean;
  title: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-pressed={on}
      disabled={disabled}
      title={title}
      onClick={onClick}
      className={cn(
        "flex h-7 cursor-pointer items-center gap-1.5 rounded-full border px-2.5 text-[12px] transition-colors disabled:cursor-default disabled:opacity-50",
        on
          ? "border-transparent bg-brand-muted font-medium text-brand"
          : "border-border bg-card text-muted-foreground hover:bg-accent hover:text-foreground",
      )}
    >
      {children}
      {on && <Check size={11} weight="bold" />}
    </button>
  );
}
