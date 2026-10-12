import { useCallback, useEffect, useState } from "react";
import { Navigate, useNavigate, useParams } from "react-router-dom";
import { Plus } from "@phosphor-icons/react";

import { LibrarySelect } from "@/components/studio/components/LibrarySelect";
import { StudioHeader } from "@/components/studio/StudioHeader";
import { InspectorToggle, useInspectorPanel } from "@/components/studio/panel/InspectorColumn";
import { DuplicateDialog } from "@/components/studio/themes/DuplicateDialog";
import { ThemeColumn } from "@/components/studio/themes/ThemeColumn";
import { ThemeEditor } from "@/components/studio/themes/ThemeEditor";
import { Button } from "@/components/ui/button";
import { LoadingFill } from "@/components/ui/layout/PageParts";
import { useComponents } from "@/hooks/studio/useComponents";
import { useLibraryChange } from "@/hooks/studio/useLibraryChange";
import { useStudioLibrary } from "@/hooks/studio/useStudioLibrary";
import { useThemeDefaults } from "@/hooks/studio/useThemeDefaults";
import { useThemes } from "@/hooks/studio/useThemes";
import { displayCode } from "@/lib/format/format";
import { THEMES_HREF, themeHref } from "@/lib/studio/routes";
import type { Tokens } from "@/lib/studio/themeDraft";
import { themeSource } from "@/lib/studio/themes";

/**
 * /studio/themes[/:name]: the themes a video can use, in the global library
 * or a subject's (one library choice with the Components page). Pick one to
 * see it on sample scenes, edit its tokens with a live preview, save, undo,
 * duplicate it, make it a library's default, or ask the agent to change it.
 */
export default function ThemesPage() {
  const { name } = useParams();
  const navigate = useNavigate();
  const { subjects, subject, code, setCode } = useStudioLibrary();
  const themes = useThemes(code);
  const defaults = useThemeDefaults(code);
  const components = useComponents(code);
  const panel = useInspectorPanel("oculus-studio-themes-inspector");
  const [revision, setRevision] = useState(0);
  const [draft, setDraft] = useState<Tokens | null>(null);
  const [fresh, setFresh] = useState<string | null>(null);

  const { reload: reloadThemes } = themes;
  const { reload: reloadDefaults } = defaults;
  const { reload: reloadComponents } = components;
  const onSettled = useCallback(() => {
    reloadThemes();
    reloadDefaults();
    void reloadComponents();
    setRevision((n) => n + 1);
  }, [reloadThemes, reloadDefaults, reloadComponents]);
  const change = useLibraryChange("theme", subject ? { id: subject.id, code: subject.code } : null, onSettled);

  const selected = name ? (themes.themes.find((t) => t.name === name) ?? null) : null;
  const names = themes.themes.map((t) => t.name);
  useEffect(() => {
    document.title = name ? `${name} · Oculus` : "Themes · Oculus";
    return () => {
      document.title = "Oculus";
    };
  }, [name]);

  // New theme starts from the built-in default's text.
  useEffect(() => {
    if (fresh !== null) return;
    themeSource("default")
      .then((s) => setFresh(s.text))
      .catch(() => {});
  }, [fresh]);
  const [creating, setCreating] = useState(false);

  if (!name && !themes.loading && themes.themes.length > 0) {
    const first = themes.themes.find((t) => t.name === defaults.defaults?.effective) ?? themes.themes[0];
    return <Navigate to={themeHref(first.name)} replace />;
  }

  return (
    <>
      <StudioHeader
        title="Themes"
        actions={
          <>
            <Button type="button" variant="outline" size="xs" onClick={() => setCreating(true)}>
              <Plus size={12} weight="bold" />
              New theme
            </Button>
            <LibrarySelect subjects={subjects} value={code} onChange={setCode} />
            <InspectorToggle panel={panel} />
          </>
        }
      />
      <div className="flex min-h-0 flex-1">
        <ThemeColumn state={themes} defaults={defaults.defaults} subject={code} open={name ?? null} draft={draft} />
        {selected ? (
          <ThemeEditor
            key={`${code ?? ""}/${selected.name}`}
            info={selected}
            subject={code}
            themeNames={names}
            components={components}
            defaults={defaults}
            revision={revision}
            change={change}
            onDraft={setDraft}
            onLibraryChanged={onSettled}
            onOpen={(next) => navigate(next ? themeHref(next) : THEMES_HREF)}
            panel={panel}
          />
        ) : (
          <div className="flex min-w-0 flex-1 items-center justify-center px-6 text-center text-[13px] text-muted-foreground">
            {themes.loading ? (
              <LoadingFill />
            ) : name ? (
              `${name} is not in ${code ? `${displayCode(code)}'s` : "the global"} library.`
            ) : null}
          </div>
        )}
      </div>
      <DuplicateDialog
        open={creating}
        title="New theme"
        suggested={names.includes("new-theme") ? "new-theme-2" : "new-theme"}
        taken={names}
        subject={code}
        text={fresh}
        onCancel={() => setCreating(false)}
        onSaved={(made) => {
          setCreating(false);
          onSettled();
          navigate(themeHref(made));
        }}
      />
    </>
  );
}
