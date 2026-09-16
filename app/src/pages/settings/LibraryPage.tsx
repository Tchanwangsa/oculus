import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { getPdfPipelineRows } from "@/lib/db";
import { tokenish } from "@/lib/parseState";
import { useParseStore } from "@/stores/parseStore";
import { cn } from "@/lib/utils";
import { EmbeddingSection } from "@/components/settings/EmbeddingSection";
import { Section, StatRow } from "./section";

interface LibraryCounts {
  tracked: number;
  parsed: number;
}

export default function SettingsLibraryPage() {
  const [library, setLibrary] = useState<LibraryCounts | null>(null);
  const [hasToken, setHasToken] = useState(false);
  const [token, setToken] = useState("");
  const [tokenNote, setTokenNote] = useState<{ kind: "error" | "warn"; text: string } | null>(null);
  const [checkingToken, setCheckingToken] = useState(false);

  // The only live signal about the token now. The sidecar used to latch a
  // rejection in its own process and report it in the health this page polled;
  // the in-process client keeps no such state (see the note in
  // `app/src-tauri/src/mineru.rs`), so what is left is the app-wide latch the
  // parse events raise. That makes the "Expired" state session-scoped rather
  // than sticky — which is the honest scope, because the very next parse reads
  // the keychain afresh and a stale flag would outlive the problem.
  const latch = useParseStore((state) => state.latch);
  const clearLatch = useParseStore((state) => state.clearLatch);

  useEffect(() => {
    let cancelled = false;
    Promise.all([getPdfPipelineRows(), invoke<boolean>("mineru_has_api_key")])
      .then(([rows, tokenPresent]) => {
        if (cancelled) return;
        setLibrary({
          tracked: rows.length,
          parsed: rows.filter((row) => row.parse_status === "quality").length,
        });
        setHasToken(tokenPresent);
      })
      .catch((error) => console.error("library settings failed", error));
    return () => {
      cancelled = true;
    };
  }, []);

  // Rust checks the token against MinerU before it reaches the keychain, so a
  // typo or an expired token is reported here rather than at the next parse.
  const saveToken = async () => {
    if (!token.trim()) return;
    setCheckingToken(true);
    setTokenNote(null);
    try {
      const verdict = await invoke<string>("mineru_set_api_key", { key: token.trim() });
      setToken("");
      setHasToken(true);
      // A token MinerU just accepted is the thing the latch was waiting on, so
      // lift it here rather than leaving the library "on hold" until some file
      // happens to parse. The sweep picks the outstanding files back up.
      clearLatch();
      setTokenNote(
        verdict === "unverified"
          ? { kind: "warn", text: "Saved, but MinerU was unreachable — it has not been checked." }
          : null,
      );
    } catch (error) {
      setTokenNote({ kind: "error", text: String(error) });
    } finally {
      setCheckingToken(false);
    }
  };

  const deleteToken = async () => {
    setTokenNote(null);
    try {
      await invoke("mineru_delete_api_key");
      setHasToken(false);
      setToken("");
    } catch (error) {
      console.error("MinerU token removal failed", error);
      setTokenNote({ kind: "error", text: String(error) });
    }
  };

  const tokenExpired = hasToken && Boolean(latch && tokenish(latch.kind, latch.message));

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

      <Section
        title="PDF processing"
        description="Every PDF is read by MinerU’s cloud service. There is no local parser."
      >
        <div className="space-y-1">
          <div className="py-2">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-xs text-foreground">MinerU API token</p>
                <p className="text-[11px] text-muted-foreground">
                  Stored in your Mac keychain, never in the library database.
                </p>
              </div>
              {hasToken && !tokenExpired ? (
                <div className="flex items-center gap-2">
                  <span className="text-xs text-success">Connected</span>
                  <Button variant="outline" size="xs" onClick={() => void deleteToken()}>
                    Remove
                  </Button>
                </div>
              ) : (
                <div className="flex items-center gap-2">
                  {tokenExpired ? <span className="text-xs text-warning">Expired</span> : null}
                  <Input
                    aria-label="MinerU API token"
                    type="password"
                    autoComplete="off"
                    value={token}
                    onChange={(event) => setToken(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") void saveToken();
                    }}
                    placeholder={tokenExpired ? "Paste new token" : "Paste token"}
                    className="h-7 w-44 text-xs"
                  />
                  <Button
                    size="xs"
                    disabled={!token.trim() || checkingToken}
                    onClick={() => void saveToken()}
                  >
                    {checkingToken ? "Checking…" : "Save"}
                  </Button>
                </div>
              )}
            </div>
            {tokenNote ? (
              <p
                className={cn(
                  "mt-2 text-[11px] leading-relaxed",
                  tokenNote.kind === "error" ? "text-destructive" : "text-warning",
                )}
              >
                {tokenNote.text}
              </p>
            ) : null}
            {tokenExpired ? (
              <p className="mt-2 text-[11px] leading-relaxed text-warning">
                MinerU refused this token during a parse. Nothing is being parsed until you paste a
                new one — there is no local parser to fall back to.
              </p>
            ) : null}
            {!hasToken && !tokenExpired ? (
              <p className="mt-2 text-[11px] text-warning">
                Without a token no PDF can be parsed, so none of them are searchable or can be
                mentioned in chat.
              </p>
            ) : null}
            <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground">
              Lecture PDFs are uploaded to MinerU and its PRC-hosted OSS storage. Results may be
              cached by MinerU (its documented default cache tolerance is 15 minutes, not a
              deletion guarantee).
            </p>
          </div>
        </div>
      </Section>

      <Separator className="my-7" />

      <EmbeddingSection />
    </>
  );
}
