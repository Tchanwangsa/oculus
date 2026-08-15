import { useState, useRef, useEffect } from "react";
import { DocumentTextIcon } from "@heroicons/react/16/solid";
import { PaperAirplaneIcon, ArrowPathIcon } from "@heroicons/react/20/solid";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { invoke } from "@tauri-apps/api/core";
import {
  embeddingStats,
  searchPages,
  type IndexStats,
  type SearchHit,
} from "@/lib/retrieval";

const SUGGESTIONS = [
  "What is the Bloch sphere representation of a qubit?",
  "When is assignment 1 due?",
  "How do I register for the QUI web interface?",
  "Do quantum gates commute — does the order matter?",
];

/** Collapse a page's markdown into a one-line preview for the result row. */
function preview(markdown: string, chars = 220): string {
  const flat = markdown
    .replace(/!\[\]\([^)]*\)/g, "") // inline images
    .replace(/[#*`>|-]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  return flat.length > chars ? `${flat.slice(0, chars)}…` : flat;
}

export default function ChatPage() {
  const [input, setInput] = useState("");
  const [hits, setHits] = useState<SearchHit[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [stats, setStats] = useState<IndexStats | null>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    embeddingStats().then(setStats).catch(() => setStats(null));
  }, []);

  async function runSearch(query: string) {
    const q = query.trim();
    if (!q || searching) return;
    setSearching(true);
    setError(null);
    try {
      setHits(await searchPages(q, 8));
    } catch (e) {
      setError(String(e));
      setHits(null);
    } finally {
      setSearching(false);
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      runSearch(input);
    }
  };

  const indexed = stats?.pages_embedded ?? 0;

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-6 h-12 border-b border-border-subtle shrink-0">
        <span className="font-semibold text-[13px] text-foreground">Chat</span>
      </div>

      {/* Results */}
      <div className="flex-1 overflow-y-auto px-6 py-6">
        {error && (
          <div className="max-w-2xl mx-auto text-sm text-destructive bg-destructive/10 border border-destructive/20 rounded-lg px-4 py-3">
            {error}
          </div>
        )}

        {!error && hits === null && (
          <div className="flex flex-col items-center justify-center h-full gap-6 text-center">
            <div className="flex flex-col items-center gap-3">
              <img src="/oculus-mark.svg" alt="" className="w-12 h-12" />
              <div>
                <h2 className="text-base font-semibold text-foreground">
                  Ask Oculus anything
                </h2>
              </div>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 w-full max-w-lg">
              {SUGGESTIONS.map((s) => (
                <button
                  key={s}
                  onClick={() => {
                    setInput(s);
                    runSearch(s);
                  }}
                  className="text-left text-xs text-muted-foreground bg-surface hover:bg-surface-raised border border-border rounded-lg px-3 py-2.5 transition-colors"
                >
                  {s}
                </button>
              ))}
            </div>
          </div>
        )}

        {!error && hits !== null && (
          <div className="max-w-2xl mx-auto flex flex-col gap-2">
            {hits.length === 0 && (
              <p className="text-sm text-muted-foreground text-center py-8">
                No matches. {indexed === 0 && "Nothing has been embedded yet."}
              </p>
            )}
            {hits.map((h) => (
              <button
                key={`${h.file_id}-${h.page_no}`}
                onClick={() =>
                  invoke("open_course_file", { relativePath: h.relative_path }).catch(
                    () => {},
                  )
                }
                className="text-left bg-surface hover:bg-surface-raised border border-border rounded-lg px-4 py-3 transition-colors"
              >
                <div className="flex items-center gap-2 mb-1">
                  <DocumentTextIcon className="size-[13px] text-muted-foreground shrink-0" />
                  <span className="text-xs font-medium text-foreground truncate">
                    {h.filename}
                  </span>
                  <Badge variant="secondary" className="shrink-0">
                    p{h.page_no}
                  </Badge>
                  <span className="text-[11px] text-muted-foreground ml-auto shrink-0 tabular-nums">
                    {h.score.toFixed(3)}
                  </span>
                </div>
                <p className="text-xs text-muted-foreground leading-relaxed">
                  {h.markdown
                    ? preview(h.markdown)
                    : "(no markdown yet — run the quality parse)"}
                </p>
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Input */}
      <div className="px-6 py-4 border-t border-border shrink-0">
        <div className="flex items-end gap-3 bg-surface rounded-xl border border-border px-4 py-3">
          <textarea
            ref={inputRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            rows={1}
            placeholder="Ask about your courses, deadlines, lectures…"
            className={cn(
              "flex-1 resize-none bg-transparent text-sm text-foreground placeholder:text-muted-foreground",
              "focus:outline-none min-h-[20px] max-h-[120px] leading-5"
            )}
            style={{ height: "20px" }}
            onInput={(e) => {
              const el = e.currentTarget;
              el.style.height = "20px";
              el.style.height = `${Math.min(el.scrollHeight, 120)}px`;
            }}
          />
          <Button
            size="icon-sm"
            disabled={!input.trim() || searching}
            onClick={() => runSearch(input)}
            className="shrink-0 mb-0.5"
            title="Search (Enter)"
          >
            {searching ? (
              <ArrowPathIcon className="size-[14px] animate-spin" />
            ) : (
              <PaperAirplaneIcon className="size-[14px]" />
            )}
          </Button>
        </div>
      </div>
    </div>
  );
}
