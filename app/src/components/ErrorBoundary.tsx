import { Component, type ReactNode } from "react";
import { useNavigate, useRouteError } from "react-router-dom";
import { Button } from "@/components/ui/button";

function text(error: unknown): string {
  const e = error as { message?: string } | null;
  return e?.message ?? String(error);
}

/** What both boundaries render: what broke, and a way out of it. */
function Fallback({ error, actions }: { error: unknown; actions: ReactNode }) {
  return (
    <div className="flex h-full w-full items-center justify-center p-8">
      <div className="max-w-md">
        <h2 className="text-sm font-medium">Something broke here</h2>
        <p className="mt-1.5 text-[13px] whitespace-pre-wrap text-muted-foreground">
          {text(error)}
        </p>
        <div className="mt-4 flex gap-2">{actions}</div>
      </div>
    </div>
  );
}

/**
 * The root route's `errorElement`, so a page that throws takes its own pane
 * and nothing else — the router stays mounted, and navigating clears it.
 * Without one, react-router paints its own unstyled stack trace there.
 */
export function RouteError() {
  const error = useRouteError();
  const navigate = useNavigate();
  return (
    <Fallback
      error={error}
      actions={
        <>
          <Button size="sm" onClick={() => navigate("/")}>
            Go home
          </Button>
          <Button size="sm" variant="outline" onClick={() => navigate(-1)}>
            Back
          </Button>
        </>
      }
    />
  );
}

/** Around the shell, which is above every router: a throw up there unmounts
 *  the tree and leaves an empty window, so catch it and offer the reload. */
export class ErrorBoundary extends Component<
  { children: ReactNode },
  { error: unknown }
> {
  state = { error: null as unknown };

  static getDerivedStateFromError(error: unknown) {
    return { error };
  }

  componentDidCatch(error: unknown) {
    console.error("shell crashed", error);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <Fallback
        error={this.state.error}
        actions={
          <Button size="sm" onClick={() => window.location.reload()}>
            Reload
          </Button>
        }
      />
    );
  }
}
