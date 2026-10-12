import { useEffect, useMemo, useState } from "react";
import { CircleNotch } from "@phosphor-icons/react";

import { CodeText } from "@/components/markdown/MdComponents";
import { ChangeBox } from "@/components/studio/components/ChangeBox";
import { InspectorColumn, stageMaxWidth, type InspectorPanel } from "@/components/studio/panel/InspectorColumn";
import { DuplicateDialog } from "@/components/studio/themes/DuplicateDialog";
import { ThemeInspector } from "@/components/studio/themes/inspector/ThemeInspector";
import { SceneStage } from "@/components/studio/themes/SceneStage";
import { ThemeHead } from "@/components/studio/themes/ThemeHead";
import { Skeleton } from "@/components/ui/skeleton";
import { PillTabs } from "@/components/ui/table/PillTabs";
import type { ComponentsState } from "@/hooks/studio/useComponents";
import type { useLibraryChange } from "@/hooks/studio/useLibraryChange";
import type { useThemeDefaults } from "@/hooks/studio/useThemeDefaults";
import { useThemeHistory, useThemeSource } from "@/hooks/studio/useThemeSource";
import type { ComponentLevel } from "@/lib/studio/components";
import { copyName, formatTheme, parseThemeText, sameTokens, type Tokens } from "@/lib/studio/themeDraft";
import { deleteTheme, restoreTheme, writeTheme, type ThemeInfo } from "@/lib/studio/themes";
import { cn } from "@/lib/utils";

type Tab = "change" | "source";
const TABS: ReadonlyArray<{ value: Tab; label: string }> = [
  { value: "change", label: "Ask for a change" },
  { value: "source", label: "Source" },
];

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/**
 * One theme open: the centre column (its name and actions, the sample scene
 * in the theme, the timeline, the change box and the file) and the token
 * inspector beside it, which folds away (`panel`) and lets the stage grow.
 * Edits make a draft
 * the stage and the list's thumbnail draw at once; Save writes it to the
 * theme's own library, and Undo writes back the snapshot that save kept.
 * The page keys it by library and theme.
 */
export function ThemeEditor({
  info,
  subject,
  themeNames,
  components,
  defaults,
  revision,
  change,
  onDraft,
  onLibraryChanged,
  onOpen,
  panel,
}: {
  info: ThemeInfo;
  /** The library's course code; null is the global library. */
  subject: string | null;
  /** Every name the library shows, which a copy must not take. */
  themeNames: string[];
  components: ComponentsState;
  defaults: ReturnType<typeof useThemeDefaults>;
  /** Bumped when the libraries changed under the page. */
  revision: number;
  change: ReturnType<typeof useLibraryChange>;
  /** The unsaved draft, for the list's thumbnail; null when there is none. */
  onDraft: (draft: Tokens | null) => void;
  /** A theme was written, removed or restored. */
  onLibraryChanged: () => void;
  /** Opens another theme (a new copy) or the list (after a delete). */
  onOpen: (name: string | null) => void;
  /** The token inspector's width and fold, held by the page for its header. */
  panel: InspectorPanel;
}) {
  const source = useThemeSource(info.name, subject, revision);
  const level: ComponentLevel | null = info.level === "bundled" ? null : info.level;
  const ownSubject = level === "subject" ? subject : null;
  const history = useThemeHistory(info.name, level, subject, revision);
  const parsed = useMemo(() => (source.source ? parseThemeText(source.source.text) : null), [source.source]);
  const base = parsed && "tokens" in parsed ? parsed.tokens : null;

  const [draft, setDraft] = useState<Tokens | null>(null);
  // A new saved text (a save, an Undo, an agent's change) replaces the draft.
  useEffect(() => setDraft(null), [source.source]);
  const tokens = draft ?? base;
  const dirty = draft != null && !sameTokens(draft, base);
  const draftText = dirty && draft ? formatTheme(draft) : null;
  useEffect(() => {
    onDraft(dirty ? draft : null);
  }, [dirty, draft, onDraft]);
  useEffect(() => () => onDraft(null), [onDraft]);

  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copying, setCopying] = useState(false);
  const [tab, setTab] = useState<Tab>("change");

  const run = async (action: () => Promise<unknown>) => {
    setSaving(true);
    setError(null);
    try {
      await action();
      onLibraryChanged();
    } catch (e) {
      setError(message(e));
    } finally {
      setSaving(false);
    }
  };
  const save = () => {
    if (!draftText) return;
    if (!level) setCopying(true);
    else void run(() => writeTheme(info.name, level, ownSubject, draftText));
  };
  const undo = () => {
    const newest = history[0];
    if (level && newest) void run(() => restoreTheme(info.name, level, ownSubject, newest.id));
  };
  const remove = async () => {
    if (!level) return;
    try {
      await deleteTheme(info.name, level, ownSubject);
      onLibraryChanged();
      onOpen(null);
    } catch (e) {
      setError(message(e));
    }
  };

  return (
    <>
      <section className="page-scroll min-h-0 min-w-0 flex-1">
        <div
          className={cn(
            "mx-auto flex w-full flex-col gap-4 px-6 py-5 transition-[max-width] duration-200 ease-out",
            stageMaxWidth(panel),
          )}
        >
          <ThemeHead
            info={info}
            subject={subject}
            dirty={dirty}
            saving={saving}
            canUndo={level != null && history.length > 0}
            onSave={save}
            onRevert={() => setDraft(null)}
            onUndo={undo}
            onDuplicate={() => setCopying(true)}
            onDelete={remove}
            onLibraryChanged={onLibraryChanged}
          />
          {(error || info.problem) && (
            <p data-selectable className="text-[12.5px] leading-snug text-destructive">
              {error ?? info.problem}
            </p>
          )}
          <SceneStage theme={info.name} draftText={draftText} subject={subject} components={components} revision={revision} />
          <div className="space-y-3 pt-1">
            <div className="flex items-center gap-3">
              <PillTabs tabs={TABS} value={tab} onChange={setTab} />
              {change.working != null && (
                <span className="flex items-center gap-1.5 text-[11.5px] text-muted-foreground">
                  <CircleNotch size={12} className="animate-spin text-brand" />
                  Changing {change.working}
                </span>
              )}
            </div>
            {tab === "source" ? (
              <SourceText text={draftText ?? source.source?.text ?? null} error={source.error} />
            ) : !level ? (
              <p className="text-[12.5px] leading-snug text-muted-foreground">
                The built-in theme does not change. Duplicate it, then ask for a change to the copy.
              </p>
            ) : dirty ? (
              <p className="text-[12.5px] leading-snug text-muted-foreground">
                Save or revert your edits before asking the agent for a change.
              </p>
            ) : (
              <ChangeBox
                name={info.name}
                working={change.working}
                applied={change.applied}
                error={change.error}
                threadId={change.threadId}
                onSend={(request) => change.send(info.name, level, request)}
                onUndo={change.undo}
              />
            )}
          </div>
        </div>
      </section>
      <InspectorColumn panel={panel} label="Theme tokens" title="Tokens">
        {source.error ? (
          <p data-selectable className="py-3 text-[12.5px] leading-snug text-destructive">
            {source.error}
          </p>
        ) : !source.source ? (
          <Skeleton className="mt-3 h-64 w-full rounded-lg" />
        ) : (
          <ThemeInspector
            name={info.name}
            level={info.level}
            subject={subject}
            tokens={tokens}
            onChange={setDraft}
            defaults={defaults.defaults}
            defaultsError={defaults.error}
            onDefault={(lvl, name) => void defaults.set(lvl, name)}
          />
        )}
      </InspectorColumn>
      <DuplicateDialog
        open={copying}
        title={dirty ? `Save ${info.name} as a copy` : `Duplicate ${info.name}`}
        suggested={copyName(info.name, themeNames)}
        taken={themeNames}
        subject={subject}
        text={draftText ?? source.source?.text ?? null}
        onCancel={() => setCopying(false)}
        onSaved={(name) => {
          setCopying(false);
          setDraft(null);
          onLibraryChanged();
          onOpen(name);
        }}
      />
    </>
  );
}

/** The theme file as saved, or as the draft would save it. */
function SourceText({ text, error }: { text: string | null; error: string | null }) {
  if (error) {
    return (
      <p data-selectable className="text-[12.5px] leading-snug text-destructive">
        {error}
      </p>
    );
  }
  if (text == null) return <Skeleton className="h-40 w-full rounded-lg" />;
  return (
    <CodeText className="max-h-96 overflow-auto whitespace-pre break-normal rounded-lg bg-secondary px-3 py-2.5">
      {text}
    </CodeText>
  );
}
