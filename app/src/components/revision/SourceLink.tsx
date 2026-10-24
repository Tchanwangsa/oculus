import { CitationLink } from "@/components/markdown/Citation";
import { parseCitation } from "@/lib/citations";
import { openLibraryPath } from "@/lib/files/openFile";
import { navigateActive } from "@/lib/shell/tabRouters";
export function SourceLink({ source }: { source: string | null }) {
  if (!source) return null;
  const shape = parseCitation(source);
  if (shape) return <CitationLink shape={shape} />;
  const open = () => {
    const chat = /^(?:chat:)?(\d+):(\d+)$/.exec(source);
    if (chat) {
      navigateActive(`/chat?t=${chat[1]}&row=${chat[2]}`);
      return;
    }
    openLibraryPath(source);
  };
  return (
    <button
      type="button"
      className="inline-flex h-5 max-w-full items-center truncate rounded-full bg-surface px-2 text-[11px] leading-none text-brand hover:underline"
      title={source}
      onClick={open}
    >
      Source · {source}
    </button>
  );
}
