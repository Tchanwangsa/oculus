import { CircleNotch } from "@phosphor-icons/react";
import { Alert, AlertDescription } from "@/components/ui/alert";

const ERRORS: Record<string, string> = {
  encrypted: "This PDF is password-protected, so it can't be shown here.",
};

/** What covers the pages until there are some: the load error, or the
 *  spinner while the document opens. */
export function PdfLoadState({ error, loading }: { error: string | null; loading: boolean }) {
  if (error) {
    return (
      <div className="absolute inset-0 flex items-center justify-center px-8 bg-card">
        {ERRORS[error] ? (
          <p className="text-sm text-muted-foreground text-center">{ERRORS[error]}</p>
        ) : (
          <Alert variant="destructive" className="w-auto">
            <AlertDescription className="text-xs">Failed to load PDF: {error}</AlertDescription>
          </Alert>
        )}
      </div>
    );
  }
  if (!loading) return null;
  return (
    <div className="absolute inset-0 flex items-start justify-center pt-8 pointer-events-none">
      <div className="flex items-center gap-2 text-muted-foreground">
        <CircleNotch size={16} className="animate-spin" />
        <span className="text-sm">Loading PDF…</span>
      </div>
    </div>
  );
}
