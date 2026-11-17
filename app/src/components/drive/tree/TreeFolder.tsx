import type { ReactNode } from "react";
import { FolderSimple, LockSimple } from "@phosphor-icons/react";
import type { DropHover } from "@/hooks/drive/useDriveDrop";
import { useFolderListing } from "@/hooks/drive/useFolderListing";
import type { FolderEntry } from "@/lib/drive/listing";
import { TreeRow } from "./TreeRow";

/** What every row of one tree shares. In a picker, `onPick` replaces
 *  navigation and only folders the student may add to can be picked. */
export interface TreeEnv {
  current: string | null;
  folderHref?: (rel: string) => string;
  onPick?: (rel: string) => void;
  open: ReadonlySet<string>;
  toggle: (rel: string) => void;
  hover: DropHover | null;
  navigate: (rel: string) => void;
}

/** A folder's row and, while open, its folders below it, read when it is
 *  first opened. Files are the browser's, never the tree's. */
export function TreeFolder({ entry, depth, env, icon, label, title, parentWritable }: {
  entry: FolderEntry;
  depth: number;
  env: TreeEnv;
  icon?: ReactNode;
  label?: string;
  title?: string;
  parentWritable: boolean;
}) {
  const expanded = env.open.has(entry.relativePath);
  const locked = parentWritable && !entry.writable;
  const glyph = icon ?? (locked
    ? <LockSimple size={14} className="text-muted-foreground" />
    : <FolderSimple size={14} weight="fill" className="text-muted-foreground" />);
  const rel = entry.relativePath;
  return (
    <>
      <TreeRow
        depth={depth}
        icon={glyph}
        label={label ?? entry.name}
        title={title}
        count={env.onPick ? undefined : entry.childCount}
        expanded={entry.childCount > 0 ? expanded : undefined}
        onToggle={() => env.toggle(rel)}
        active={env.current === rel}
        disabled={env.onPick ? !entry.writable : false}
        href={env.onPick ? undefined : env.folderHref?.(rel)}
        onActivate={() => (env.onPick ? env.onPick(rel) : env.navigate(rel))}
        drop={{ rel, writable: entry.writable }}
        hover={env.hover}
      />
      {expanded && <TreeChildren rel={rel} depth={depth + 1} env={env} writable={entry.writable} />}
    </>
  );
}

function TreeChildren({ rel, depth, env, writable }: { rel: string; depth: number; env: TreeEnv; writable: boolean }) {
  const { listing } = useFolderListing(rel);
  if (!listing) return null;
  return (
    <>
      {listing.folders.map((folder) => (
        <TreeFolder key={folder.relativePath} entry={folder} depth={depth} env={env} parentWritable={writable} />
      ))}
    </>
  );
}
