import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";

export function EmptyState() {
  const navigate = useNavigate();
  return (
    <div className="rounded-lg border border-border px-4 py-6 text-center">
      <p className="text-sm text-foreground">Nothing synced for this subject yet.</p>
      <p className="text-xs text-muted-foreground mt-1">
        Run a sync to pull its modules, pages, files and lectures.
      </p>
      <Button
        size="sm"
        variant="outline"
        className="mt-3 text-xs"
        onClick={() => navigate("/sync")}
      >
        Go to Sync
      </Button>
    </div>
  );
}
