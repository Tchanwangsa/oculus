import {
  createContext, useContext, useState, useCallback, useRef, type ReactNode,
} from "react";
import { CheckCircle2, AlertCircle, Loader2, X } from "lucide-react";
import { cn } from "@/lib/utils";

// ── Types ──────────────────────────────────────────────────────────────────

export type ToastKind = "info" | "success" | "error" | "progress";

export interface ToastData {
  id: string;
  kind: ToastKind;
  title: string;
  /** Optional sub-line (e.g. "Lecture5.pdf"). */
  detail?: string;
  /** 0–100 for progress toasts. Omit for indeterminate spinner. */
  progress?: number;
  /** Auto-dismiss after N ms. 0 / undefined = sticky (caller must dismiss). */
  duration?: number;
}

interface ToastContextValue {
  /** Create or update a toast. Pass an existing id to update in place. */
  push: (toast: Omit<ToastData, "id"> & { id?: string }) => string;
  dismiss: (id: string) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export function useToast(): ToastContextValue {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error("useToast must be used within ToastProvider");
  return ctx;
}

// ── Provider ───────────────────────────────────────────────────────────────

let _seq = 0;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastData[]>([]);
  const timers = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  const dismiss = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
    const tm = timers.current.get(id);
    if (tm) { clearTimeout(tm); timers.current.delete(id); }
  }, []);

  const push = useCallback((toast: Omit<ToastData, "id"> & { id?: string }) => {
    const id = toast.id ?? `toast-${++_seq}`;
    setToasts((prev) => {
      const existing = prev.find((t) => t.id === id);
      const next: ToastData = { ...existing, ...toast, id };
      return existing ? prev.map((t) => (t.id === id ? next : t)) : [...prev, next];
    });

    // (Re)arm auto-dismiss
    const tm = timers.current.get(id);
    if (tm) clearTimeout(tm);
    if (toast.duration && toast.duration > 0) {
      timers.current.set(id, setTimeout(() => dismiss(id), toast.duration));
    } else {
      timers.current.delete(id);
    }
    return id;
  }, [dismiss]);

  return (
    <ToastContext.Provider value={{ push, dismiss }}>
      {children}
      <ToastViewport toasts={toasts} onDismiss={dismiss} />
    </ToastContext.Provider>
  );
}

// ── Viewport + Toast ───────────────────────────────────────────────────────

function ToastViewport({ toasts, onDismiss }: { toasts: ToastData[]; onDismiss: (id: string) => void }) {
  if (toasts.length === 0) return null;
  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2 w-80 pointer-events-none">
      {toasts.map((t) => (
        <ToastItem key={t.id} toast={t} onDismiss={() => onDismiss(t.id)} />
      ))}
    </div>
  );
}

function ToastItem({ toast, onDismiss }: { toast: ToastData; onDismiss: () => void }) {
  const { kind, title, detail, progress } = toast;
  return (
    <div className="pointer-events-auto rounded-lg border border-border bg-surface-raised shadow-lg px-3 py-2.5 flex items-start gap-2.5 animate-in slide-in-from-right-4 fade-in duration-200">
      <div className="shrink-0 mt-0.5">
        {kind === "success" && <CheckCircle2 size={15} className="text-success" />}
        {kind === "error" && <AlertCircle size={15} className="text-destructive" />}
        {(kind === "progress" || kind === "info") && (
          <Loader2 size={15} className="text-primary animate-spin" />
        )}
      </div>

      <div className="flex-1 min-w-0">
        <p className="text-xs font-medium text-foreground leading-tight">{title}</p>
        {detail && (
          <p className="text-[11px] text-muted-foreground truncate mt-0.5">{detail}</p>
        )}
        {kind === "progress" && progress !== undefined && (
          <div className="mt-1.5 h-1 rounded-full bg-border overflow-hidden">
            <div
              className="h-full bg-primary transition-all duration-500 rounded-full"
              style={{ width: `${Math.max(2, Math.min(100, progress))}%` }}
            />
          </div>
        )}
      </div>

      <button
        onClick={onDismiss}
        className={cn(
          "shrink-0 p-0.5 rounded text-muted-foreground/60 hover:text-foreground transition-colors",
        )}
      >
        <X size={12} />
      </button>
    </div>
  );
}
