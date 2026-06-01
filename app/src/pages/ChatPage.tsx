import { useState, useRef, useEffect } from "react";
import { Send, Sparkles, Network, BookOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

const TOOLS = [
  { icon: Network, label: "Knowledge Graph" },
  { icon: BookOpen, label: "Subjects" },
];

const SUGGESTIONS = [
  "Summarise Week 3 lectures for SWEN30006",
  "What assignments are due this week?",
  "Explain the concept of mutual exclusion from COMP30023",
  "Compare the ML models covered so far in COMP30027",
];

export default function ChatPage() {
  const [input, setInput] = useState("");
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      // send message — wired up when backend ready
    }
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-6 h-14 border-b border-border shrink-0">
        <div className="flex items-center gap-2">
          <Sparkles size={16} className="text-primary" />
          <span className="font-semibold text-foreground">Chat</span>
          <Badge variant="secondary">Knowledge Graph</Badge>
        </div>
        <div className="flex items-center gap-1.5">
          {TOOLS.map(({ icon: Icon, label }) => (
            <button
              key={label}
              title={label}
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs text-muted-foreground hover:text-foreground hover:bg-surface transition-colors"
            >
              <Icon size={13} />
              <span className="hidden sm:inline">{label}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Messages area */}
      <div className="flex-1 overflow-y-auto px-6 py-6">
        {/* Empty state */}
        <div className="flex flex-col items-center justify-center h-full gap-6 text-center">
          <div className="flex flex-col items-center gap-3">
            <div className="w-14 h-14 rounded-2xl bg-primary/10 flex items-center justify-center">
              <Sparkles size={24} className="text-primary" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-foreground">Ask Oculus anything</h2>
              <p className="text-sm text-muted-foreground mt-1 max-w-xs">
                Reason over your lecture slides, assignments, and notes — all from your personal knowledge graph.
              </p>
            </div>
          </div>

          {/* Suggestions */}
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 w-full max-w-lg">
            {SUGGESTIONS.map((s) => (
              <button
                key={s}
                onClick={() => setInput(s)}
                className="text-left text-xs text-muted-foreground bg-surface hover:bg-surface-raised border border-border rounded-lg px-3 py-2.5 transition-colors"
              >
                {s}
              </button>
            ))}
          </div>
        </div>
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
            disabled={!input.trim()}
            className="shrink-0 mb-0.5"
            title="Send (Enter)"
          >
            <Send size={14} />
          </Button>
        </div>
        <p className="text-[11px] text-muted-foreground mt-2 text-center">
          Enter to send · Shift+Enter for new line
        </p>
      </div>
    </div>
  );
}
