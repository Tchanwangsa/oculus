import { useEffect, useState } from "react";
import { Separator } from "@/components/ui/separator";
import { getPdfPipelineRows } from "@/lib/db";
import { ParserSection } from "@/components/settings/ParserSection";
import { Section, StatRow } from "./section";

interface PipelineCounts {
  tracked: number;
  parsed: number;
}

export default function SettingsParsingPage() {
  const [counts, setCounts] = useState<PipelineCounts | null>(null);

  useEffect(() => {
    let cancelled = false;

    getPdfPipelineRows()
      .then((rows) => {
        if (cancelled) return;
        setCounts({
          tracked: rows.length,
          parsed: rows.filter((row) => row.parse_status === "quality").length,
        });
      })
      .catch((error) => console.error("pipeline counts failed", error));

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <>
      <Section title="Pipeline" description="Where synced PDFs are in the parse pipeline.">
        <div>
          <StatRow label="PDFs tracked" value={counts ? String(counts.tracked) : "—"} />
          <StatRow
            label="Parsed"
            value={counts ? `${counts.parsed}/${counts.tracked}` : "—"}
          />
        </div>
      </Section>

      <Separator className="my-7" />

      <ParserSection />
    </>
  );
}
