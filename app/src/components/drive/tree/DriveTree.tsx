import { useMemo, useRef } from "react";
import { Archive } from "@phosphor-icons/react";
import { SideNavGroupLabel } from "@/components/ui/layout/SideNav";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { useSubjects } from "@/hooks/data/useSubjects";
import { useDriveDrop } from "@/hooks/drive/useDriveDrop";
import { useFolderListing } from "@/hooks/drive/useFolderListing";
import { useStoredSet } from "@/hooks/ui/useStoredState";
import { dropInto, importInto, reportDriveProblem } from "@/lib/drive/actions";
import { READ_ONLY_TEXT } from "@/lib/drive/commands";
import { subjectForFolder } from "@/lib/drive/paths";
import { displayCode, displayName } from "@/lib/format/format";
import { navigateActive } from "@/lib/shell/tabRouters";
import { TreeFolder, type TreeEnv } from "./TreeFolder";
import { TreeRow } from "./TreeRow";

const PAST = ":past";

/**
 * Drive's folder tree, for its nav column: every subject with a folder,
 * current ones under Subjects and the rest folded under Past subjects, each
 * opening in place to its folders (read when first opened). Which rows are
 * open is remembered (`useStoredSet`). Rows are drop targets for Finder files
 * and for entries dragged from the browser, as its folder cards are.
 *
 * With `onPick` it is a folder picker over one subject (`root`): rows pick
 * instead of navigating, and folders the student can't add to are off.
 */
export function DriveTree({
  current,
  folderHref,
  onPick,
  root,
  storageKey = "oculus-drive-tree-open",
}: {
  /** The folder on screen (or picked), highlighted. */
  current: string | null;
  folderHref?: (rel: string) => string;
  onPick?: (rel: string) => void;
  /** A subject folder (`courses/<code>`) to show alone; the Drive root when unset. */
  root?: string;
  storageKey?: string;
}) {
  const { subjects } = useSubjects();
  const [open, setOpen] = useStoredSet(storageKey);
  const ref = useRef<HTMLDivElement>(null);
  const { hover, handlers } = useDriveDrop(ref, {
    fallback: null,
    onFiles: (paths, target) => void importInto(paths, target.rel, subjects),
    onEntries: (plan, target) => void dropInto(plan, target.rel, subjects),
    onRefused: () => reportDriveProblem(READ_ONLY_TEXT),
  });
  // A picker opens on its subject's folders.
  const shown = useMemo(() => (root ? new Set([...open, root]) : open), [open, root]);
  const env: TreeEnv = {
    current,
    folderHref,
    onPick,
    open: shown,
    toggle: (rel) => setOpen((prev) => {
      const next = new Set(prev);
      if (next.has(rel)) next.delete(rel);
      else next.add(rel);
      return next;
    }),
    hover,
    navigate: (rel) => folderHref && navigateActive(folderHref(rel)),
  };

  const { listing } = useFolderListing("");
  const folders = useMemo(() => {
    const all = listing?.folders ?? [];
    return root == null ? all : all.filter((f) => f.relativePath === root);
  }, [listing, root]);
  const withSubject = folders.map((entry) => ({ entry, subject: subjectForFolder(entry.name, subjects) }));
  const currentOnes = root == null ? withSubject.filter((s) => s.subject?.is_current) : withSubject;
  const pastOnes = root == null ? withSubject.filter((s) => !s.subject?.is_current) : [];

  const subjectRow = ({ entry, subject }: (typeof withSubject)[number], depth = 0) => (
    <TreeFolder
      key={entry.relativePath}
      entry={entry}
      depth={depth}
      env={env}
      parentWritable={false}
      icon={subject ? <SubjectIcon code={subject.code} size={14} /> : undefined}
      label={subject ? displayCode(subject.code) : entry.name}
      title={subject ? displayName(subject.name, subject.code) : entry.name}
    />
  );

  return (
    <div ref={ref} role="tree" aria-label="Folders" {...handlers}>
      {root == null && currentOnes.length > 0 && <SideNavGroupLabel>Subjects</SideNavGroupLabel>}
      {currentOnes.map((s) => subjectRow(s))}
      {pastOnes.length > 0 && (
        <>
          <TreeRow
            depth={0}
            icon={<Archive size={14} />}
            label="Past subjects"
            expanded={open.has(PAST)}
            onToggle={() => env.toggle(PAST)}
            onActivate={() => env.toggle(PAST)}
            active={false}
            hover={null}
          />
          {open.has(PAST) && pastOnes.map((s) => subjectRow(s, 1))}
        </>
      )}
    </div>
  );
}
