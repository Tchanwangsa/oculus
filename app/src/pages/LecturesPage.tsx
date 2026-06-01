import { Play, Download, Clock, Sparkles, ChevronRight, FileVideo } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";

const COURSES = [
  {
    code: "COMP30023",
    name: "Computer Systems",
    lectures: [
      { id: 1, title: "Introduction & Network Layers", duration: "1h 12m", week: 1, downloaded: true },
      { id: 2, title: "TCP/IP and Addressing",          duration: "1h 08m", week: 2, downloaded: true },
      { id: 3, title: "Routing Algorithms",             duration: "1h 15m", week: 3, downloaded: false },
    ],
  },
  {
    code: "SWEN30006",
    name: "Software Modelling",
    lectures: [
      { id: 4, title: "UML Class Diagrams",     duration: "58m",   week: 1, downloaded: true },
      { id: 5, title: "Design Patterns I",      duration: "1h 04m", week: 2, downloaded: false },
    ],
  },
  {
    code: "COMP30027",
    name: "Machine Learning",
    lectures: [
      { id: 6, title: "Probability & Bayes",    duration: "1h 20m", week: 1, downloaded: true },
      { id: 7, title: "Decision Trees",         duration: "1h 10m", week: 2, downloaded: false },
    ],
  },
];

export default function LecturesPage() {
  const firstLecture = COURSES[0].lectures[0];

  return (
    <div className="flex h-full">
      {/* Lecture list sidebar */}
      <div className="w-72 shrink-0 border-r border-border flex flex-col h-full overflow-hidden">
        <div className="px-4 h-14 flex items-center border-b border-border shrink-0">
          <span className="font-semibold text-foreground text-sm">Lectures</span>
        </div>
        <div className="flex-1 overflow-y-auto py-2">
          {COURSES.map((course) => (
            <div key={course.code} className="mb-1">
              <div className="px-4 py-2 flex items-center justify-between">
                <span className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                  {course.code}
                </span>
                <ChevronRight size={12} className="text-muted-foreground" />
              </div>
              {course.lectures.map((lec) => (
                <button
                  key={lec.id}
                  className={cn(
                    "w-full text-left px-4 py-2.5 flex items-start gap-3 hover:bg-surface transition-colors",
                    lec.id === firstLecture.id && "bg-surface-raised"
                  )}
                >
                  <FileVideo size={14} className="shrink-0 mt-0.5 text-muted-foreground" />
                  <div className="min-w-0">
                    <p className="text-xs font-medium text-foreground truncate">{lec.title}</p>
                    <div className="flex items-center gap-2 mt-0.5">
                      <span className="text-[11px] text-muted-foreground">Week {lec.week}</span>
                      <span className="text-[11px] text-muted-foreground flex items-center gap-0.5">
                        <Clock size={10} /> {lec.duration}
                      </span>
                      {!lec.downloaded && (
                        <span className="text-[10px] text-warning font-medium">Not synced</span>
                      )}
                    </div>
                  </div>
                </button>
              ))}
              <Separator className="my-1" />
            </div>
          ))}
        </div>
      </div>

      {/* Player area */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Video player */}
        <div className="flex-1 bg-black flex items-center justify-center relative min-h-0">
          <div className="flex flex-col items-center gap-4 text-white/60">
            <div className="w-20 h-20 rounded-full bg-white/10 flex items-center justify-center">
              <Play size={32} className="text-white ml-1" />
            </div>
            <div className="text-center">
              <p className="text-sm font-medium text-white">{firstLecture.title}</p>
              <p className="text-xs text-white/50 mt-0.5">COMP30023 · Week 1 · {firstLecture.duration}</p>
            </div>
            <button className="flex items-center gap-2 px-4 py-2 rounded-lg bg-white/10 hover:bg-white/20 text-sm font-medium text-white transition-colors">
              <Play size={14} /> Play lecture
            </button>
          </div>
          {/* Download badge overlay */}
          <div className="absolute top-4 right-4">
            <Badge variant="success">Downloaded</Badge>
          </div>
        </div>

        {/* AI assistant panel */}
        <div className="h-52 shrink-0 border-t border-border flex flex-col bg-background">
          <div className="px-5 h-11 flex items-center gap-2 border-b border-border">
            <Sparkles size={14} className="text-primary" />
            <span className="text-xs font-semibold text-foreground">AI Assistant</span>
            <span className="text-xs text-muted-foreground ml-auto">Context: this lecture</span>
          </div>
          <div className="flex-1 overflow-y-auto px-5 py-3">
            <div className="flex flex-col items-center justify-center h-full gap-2 text-center">
              <p className="text-xs text-muted-foreground">
                Play a lecture and ask questions about what you&apos;re watching.
              </p>
              <div className="flex gap-2 flex-wrap justify-center">
                {["Explain this concept", "Summarise so far", "Quiz me"].map((s) => (
                  <button
                    key={s}
                    className="text-xs px-2.5 py-1 rounded-full border border-border text-muted-foreground hover:text-foreground hover:bg-surface transition-colors"
                  >
                    {s}
                  </button>
                ))}
              </div>
            </div>
          </div>
          <div className="px-5 py-2.5 border-t border-border">
            <div className="flex gap-2 items-center bg-surface rounded-lg border border-border px-3 py-2">
              <input
                placeholder="Ask about this lecture…"
                className="flex-1 bg-transparent text-xs text-foreground placeholder:text-muted-foreground focus:outline-none"
              />
              <button className="text-primary hover:text-primary/80 transition-colors">
                <Download size={13} />
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
