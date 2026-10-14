import { useEffect, useState } from "react";

import { LazyCodeEditor } from "@/components/code/LazyCodeEditor";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { LoadingFill } from "@/components/ui/layout/PageParts";
import { PillTabs } from "@/components/ui/table/PillTabs";
import { submissionFiles, type SubmissionFile } from "@/lib/ed/workspace";

/** A submission's code as it went in: its files in tabs, read-only. */
export function SubmissionCode({ challengeId, submissionId }: { challengeId: number; submissionId: number }) {
  const [files, setFiles] = useState<SubmissionFile[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [active, setActive] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    setFiles(null);
    setError(null);
    setActive(null);
    submissionFiles(challengeId, submissionId)
      .then((read) => live && setFiles(read))
      .catch((e) => live && setError(String(e)));
    return () => {
      live = false;
    };
  }, [challengeId, submissionId]);

  if (error)
    return (
      <Alert variant="destructive">
        <AlertTitle className="text-xs">Couldn't load the submission's code</AlertTitle>
        <AlertDescription className="text-xs">{error}</AlertDescription>
      </Alert>
    );
  if (files === null)
    return (
      <div className="h-24">
        <LoadingFill />
      </div>
    );
  if (files.length === 0) return <p className="text-sm text-muted-foreground">This submission has no files.</p>;
  const shown = files.find((f) => f.path === active) ?? files[0];
  return (
    <div className="space-y-2">
      <PillTabs scroll tabs={files.map((f) => ({ value: f.path, label: f.path }))} value={shown.path} onChange={setActive} />
      <div className="h-96 overflow-hidden rounded-md border border-border-subtle">
        <LazyCodeEditor key={shown.path} text={shown.text} filename={shown.path} readOnly />
      </div>
    </div>
  );
}
