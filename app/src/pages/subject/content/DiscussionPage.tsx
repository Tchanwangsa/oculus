import { useDeferredValue, useMemo, useRef, useState } from "react";
import { ChatsCircle } from "@phosphor-icons/react";
import { SubjectLoading, SubjectEmpty } from "@/components/subjects/SubjectPage";
import { useSubjectFiles } from "@/hooks/data/useSubjectFiles";
import { useSubject } from "@/layouts/SubjectLayout";
import { useCourseFileData } from "@/hooks/data/useCourseFileData";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { filterEdThreads, parseEdThread, type ThreadFilters as Filters } from "@/lib/ed/threads";
import { ThreadFilters } from "./discussion/ThreadFilters";
import { THREAD_COLUMNS, ThreadRow } from "./discussion/ThreadRow";

/** The synced Ed board: local keyword search includes the thread's replies. */
export default function SubjectDiscussionPage() {
  const subject = useSubject();
  const { byCategory, loading } = useSubjectFiles(subject.id);
  const files = useMemo(() => [...byCategory.ed].sort((a, b) =>
    b.filename.localeCompare(a.filename, undefined, { numeric: true })), [byCategory.ed]);
  const entries = useCourseFileData(files, loading, parseEdThread);
  const threads = useMemo(() => {
    const byId = new Map(entries?.map((thread) => [thread.file.id, thread]));
    return files.map((file) => byId.get(file.id) ?? parseEdThread("", file));
  }, [entries, files]);
  const [filters, setFilters] = useState<Filters>(() => ({
    query: "", types: new Set(), statuses: new Set(), categories: new Set(),
  }));
  const query = useDeferredValue(filters.query);
  const shown = useMemo(() => filterEdThreads(threads, { ...filters, query }), [threads, filters, query]);
  const scrollRef = useRef<HTMLDivElement>(null);
  useScrollFade(scrollRef, "y", shown.length > 0);
  const reading = loading || entries == null;

  if (loading && files.length === 0) return <SubjectLoading count={5} />;
  if (files.length === 0) return (
    <SubjectEmpty icon={<ChatsCircle size={24} className="text-muted-foreground/40" />} title="No Ed threads scraped.">
      <p className="max-w-sm text-center text-xs text-muted-foreground/70">
        Run a sync — Ed connects automatically through your Canvas session
        when the subject has an Ed Discussion board.
      </p>
    </SubjectEmpty>
  );

  return (
    <div className="@container flex h-full min-w-0 flex-col">
      <ThreadFilters threads={threads} filters={filters} onChange={setFilters} shown={shown.length} loading={reading} />
      <div className="shrink-0 px-6 pr-[30px]">
        <div className={`${THREAD_COLUMNS} border-b border-border-subtle py-2 text-[11px] font-medium text-muted-foreground`}>
          <span>Thread</span>
          <span className="hidden @[800px]:block">Category</span>
          <span className="hidden @[600px]:block">Status</span>
          <span className="text-right">Posted</span>
          <span className="hidden text-right @[600px]:block">Activity</span>
        </div>
      </div>
      <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-scroll px-6 pb-5">
        {shown.map((thread) => <ThreadRow key={thread.file.id} thread={thread} />)}
        {!reading && shown.length === 0 && (
          <div className="flex flex-col items-center gap-2 py-16 text-center">
            <p className="text-sm text-foreground">No matching threads</p>
            <p className="text-xs text-muted-foreground">Try different keywords or clear your filters.</p>
            <button type="button" onClick={() => setFilters({ query: "", types: new Set(), statuses: new Set(), categories: new Set() })} className="mt-2 rounded-full px-3 py-1.5 text-xs text-brand hover:bg-brand/10">Clear search and filters</button>
          </div>
        )}
      </div>
    </div>
  );
}
