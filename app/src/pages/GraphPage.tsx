import { Network, Search, ZoomIn, ZoomOut, Maximize2, Filter, Info } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";

const GRAPH_STATS = [
  { label: "Nodes", value: "0" },
  { label: "Edges", value: "0" },
  { label: "Courses", value: "3" },
];

const NODE_TYPES = [
  { label: "Lecture",     color: "#73c6c2" },
  { label: "Assignment",  color: "#5da0ff" },
  { label: "Topic",       color: "#f59e0b" },
  { label: "Announcement",color: "#22c55e" },
  { label: "Staff",       color: "#a78bfa" },
];

export default function GraphPage() {
  return (
    <div className="flex flex-col h-full">
      {/* Toolbar */}
      <div className="flex items-center gap-3 px-5 h-14 border-b border-border shrink-0">
        <div className="flex items-center gap-2">
          <Network size={16} className="text-primary" />
          <span className="font-semibold text-foreground text-sm">Knowledge Graph</span>
        </div>

        <div className="flex items-center gap-1.5 ml-auto">
          {/* Search */}
          <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-surface border border-border">
            <Search size={13} className="text-muted-foreground" />
            <input
              placeholder="Search nodes…"
              className="bg-transparent text-xs text-foreground placeholder:text-muted-foreground focus:outline-none w-36"
            />
          </div>

          <Button variant="ghost" size="icon-sm" title="Filter">
            <Filter size={14} />
          </Button>
          <Button variant="ghost" size="icon-sm" title="Zoom in">
            <ZoomIn size={14} />
          </Button>
          <Button variant="ghost" size="icon-sm" title="Zoom out">
            <ZoomOut size={14} />
          </Button>
          <Button variant="ghost" size="icon-sm" title="Fit to screen">
            <Maximize2 size={14} />
          </Button>
        </div>
      </div>

      <div className="flex-1 flex overflow-hidden">
        {/* Canvas */}
        <div className="flex-1 bg-surface/50 flex items-center justify-center relative overflow-hidden">
          {/* Empty state */}
          <div className="flex flex-col items-center gap-5 text-center max-w-sm">
            <div className="w-16 h-16 rounded-2xl bg-primary/10 flex items-center justify-center">
              <Network size={28} className="text-primary" />
            </div>
            <div>
              <h2 className="text-base font-semibold text-foreground">No graph data yet</h2>
              <p className="text-sm text-muted-foreground mt-1.5">
                Sync your Canvas subjects to build the knowledge graph. Nodes represent pages,
                edges represent links and relationships.
              </p>
            </div>
            <div className="flex gap-2">
              {GRAPH_STATS.map(({ label, value }) => (
                <div key={label} className="px-4 py-2.5 rounded-lg bg-card border border-border text-center">
                  <p className="text-lg font-bold text-foreground">{value}</p>
                  <p className="text-[11px] text-muted-foreground mt-0.5">{label}</p>
                </div>
              ))}
            </div>
          </div>

          {/* Grid overlay hint */}
          <div
            className="absolute inset-0 pointer-events-none opacity-30"
            style={{
              backgroundImage:
                "radial-gradient(circle, var(--color-border) 1px, transparent 1px)",
              backgroundSize: "28px 28px",
            }}
          />
        </div>

        {/* Legend + info panel */}
        <div className="w-56 shrink-0 border-l border-border flex flex-col overflow-hidden">
          <div className="px-4 py-3 border-b border-border">
            <p className="text-xs font-semibold text-foreground">Node Types</p>
          </div>
          <div className="flex-1 overflow-y-auto px-4 py-3 space-y-2">
            {NODE_TYPES.map(({ label, color }) => (
              <div key={label} className="flex items-center gap-2.5">
                <div
                  className="w-3 h-3 rounded-full shrink-0"
                  style={{ backgroundColor: color }}
                />
                <span className="text-xs text-muted-foreground">{label}</span>
              </div>
            ))}
          </div>

          {/* Selected node info placeholder */}
          <div className="border-t border-border p-4">
            <div className="flex items-center gap-1.5 mb-2">
              <Info size={12} className="text-muted-foreground" />
              <p className="text-xs font-medium text-muted-foreground">Node Details</p>
            </div>
            <p className="text-xs text-muted-foreground">
              Click a node to inspect its properties and connected edges.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
