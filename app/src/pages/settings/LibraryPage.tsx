import { useEffect, useState } from "react";
import { Separator } from "@/components/ui/separator";
import { getPdfPipelineRows } from "@/lib/db";
import { EmbeddingSection } from "@/components/settings/EmbeddingSection";
import { ParserSection } from "@/components/settings/ParserSection";
import { Section, StatRow } from "./section";

interface LibraryCounts {
  tracked: number;
  parsed: number;
}

export default function SettingsLibraryPage() {
  const [library, setLibrary] = useState<LibraryCounts | null>(null);

  useEffect(() => {
    let cancelled = false;

    getPdfPipelineRows()
      .then((rows) => {
        if (cancelled) return;
        setLibrary({
          tracked: rows.length,
          parsed: rows.filter((row) => row.parse_status === "quality").length,
        });
      })
      .catch((error) => console.error("library counts failed", error));

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <>
      <Section title="Library" description="Where synced PDFs are in the parse pipeline.">
        <div>
          <StatRow label="PDFs tracked" value={library ? String(library.tracked) : "—"} />
          <StatRow
            label="Parsed"
            value={library ? `${library.parsed}/${library.tracked}` : "—"}
          />
        </div>
      </Section>

      <Separator className="my-7" />

      <ParserSection />

      <Separator className="my-7" />

      <EmbeddingSection />
    </>
  );
}
