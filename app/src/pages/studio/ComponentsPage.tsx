import { useCallback, useEffect, useState } from "react";
import { Navigate, useParams } from "react-router-dom";

import { ComponentList } from "@/components/studio/components/ComponentList";
import { LibrarySelect } from "@/components/studio/components/LibrarySelect";
import { Workbench } from "@/components/studio/components/Workbench";
import { InspectorToggle, useInspectorPanel } from "@/components/studio/panel/InspectorColumn";
import { StudioHeader } from "@/components/studio/StudioHeader";
import { LoadingFill } from "@/components/ui/layout/PageParts";
import { useComponents } from "@/hooks/studio/useComponents";
import { useLibraryChange } from "@/hooks/studio/useLibraryChange";
import { useStudioLibrary } from "@/hooks/studio/useStudioLibrary";
import { useThemes } from "@/hooks/studio/useThemes";
import { useStoredState } from "@/hooks/ui/useStoredState";
import { displayCode } from "@/lib/format/format";
import { componentHref } from "@/lib/studio/routes";

const readStored = (raw: string | null) => raw ?? "";

/**
 * /studio/components[/:name]: the components a video could use, in the
 * global library or a subject's. Pick one to draw it in a theme, play or
 * scrub it, change its props, read its TSX, or ask the agent to change it.
 */
export default function ComponentsPage() {
  const { name } = useParams();
  const { subjects, subject, code, setCode } = useStudioLibrary();
  const [storedTheme, setTheme] = useStoredState("oculus-studio-components-theme", readStored);
  const [revision, setRevision] = useState(0);
  const panel = useInspectorPanel("oculus-studio-components-inspector");
  const { components, loaded, error, reload } = useComponents(code);
  const themes = useThemes(code);
  const theme = themes.themes.find((t) => t.name === storedTheme && !t.problem)?.name
    ?? themes.themes.find((t) => !t.problem)?.name
    ?? null;

  const onSettled = useCallback(() => {
    void reload();
    setRevision((n) => n + 1);
  }, [reload]);
  const change = useLibraryChange("component", subject ? { id: subject.id, code: subject.code } : null, onSettled);

  const selected = name ? (components.find((c) => c.name === name) ?? null) : null;
  useEffect(() => {
    document.title = name ? `${name} · Oculus` : "Components · Oculus";
    return () => {
      document.title = "Oculus";
    };
  }, [name]);

  if (!name && loaded && components.length > 0) {
    return <Navigate to={componentHref(components[0].name)} replace />;
  }

  return (
    <>
      <StudioHeader
        title="Components"
        actions={
          <>
            <LibrarySelect subjects={subjects} value={code} onChange={setCode} />
            <InspectorToggle panel={panel} />
          </>
        }
      />
      <div className="flex min-h-0 flex-1">
        <ComponentList components={components} loaded={loaded} error={error} subject={code} />
        {selected ? (
          <Workbench
            key={`${code ?? ""}/${selected.name}`}
            info={selected}
            subject={code}
            themes={themes}
            theme={theme}
            onTheme={setTheme}
            revision={revision}
            change={change}
            onLibraryChanged={onSettled}
            panel={panel}
          />
        ) : (
          <div className="flex min-w-0 flex-1 items-center justify-center px-6 text-center text-[13px] text-muted-foreground">
            {!loaded ? (
              <LoadingFill />
            ) : name ? (
              `${name} is not in ${code ? `${displayCode(code)}'s` : "the global"} library.`
            ) : null}
          </div>
        )}
      </div>
    </>
  );
}
