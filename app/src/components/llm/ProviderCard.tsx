import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, PencilSimple, Trash, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { providerNeedsKey, PROVIDER_KINDS, type LlmProvider } from "@/lib/db";

/** Placeholder base URL for a kind — the value Rust falls back to when the
 *  field is left blank (`ProviderConfig::base_url`). */
export const DEFAULT_BASE_URL: Record<string, string> = {
  ollama: "http://localhost:11434",
  lmstudio: "http://localhost:1234",
  openrouter: "https://openrouter.ai/api",
  "opencode-go": "https://opencode.ai/zen/go",
  custom: "https://api.example.com",
};

/** Store the endpoint root without its `/v1` suffix — the request builder adds
 *  one, and providers advertise their URL both ways. Mirrors the same trim in
 *  `ProviderConfig::base_url` (`app/src-tauri/src/llm.rs`), which stays the
 *  authority for settings written before this. */
export function normalizeBaseUrl(url: string): string {
  const trimmed = url.trim().replace(/\/+$/, "");
  return trimmed.replace(/\/v1$/, "");
}

/** One configured endpoint. Read-only until Edit is pressed: a provider is set
 *  up once and then only looked at, so the resting state is a line of text, not
 *  a row of live inputs. The key itself is never read back — only its presence. */
export function ProviderCard({
  provider,
  onChange,
  onRemove,
}: {
  provider: LlmProvider;
  onChange: (patch: Partial<LlmProvider>) => void;
  onRemove: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [hasKey, setHasKey] = useState(false);
  const [keyDraft, setKeyDraft] = useState("");
  const needsKey = providerNeedsKey(provider.kind);
  const kindLabel = PROVIDER_KINDS.find((p) => p.kind === provider.kind)?.label;
  // Shown normalized, so the line matches the URL calls actually go to even
  // for a provider saved with a `/v1` suffix before the trim existed.
  const baseUrl = provider.baseUrl
    ? normalizeBaseUrl(provider.baseUrl)
    : DEFAULT_BASE_URL[provider.kind];

  useEffect(() => {
    invoke<boolean>("llm_has_api_key", { providerId: provider.id })
      .then(setHasKey)
      .catch(() => {});
  }, [provider.id]);

  const saveKey = async () => {
    const key = keyDraft.trim();
    if (!key) return;
    await invoke("llm_set_api_key", { providerId: provider.id, key });
    setKeyDraft("");
    setHasKey(true);
  };

  const removeKey = async () => {
    await invoke("llm_delete_api_key", { providerId: provider.id });
    setHasKey(false);
  };

  if (!editing) {
    return (
      <div className="rounded-lg border border-border bg-surface px-3 py-2.5 flex items-center gap-2">
        <div className="min-w-0 flex flex-col gap-0.5">
          <div className="flex items-center gap-2">
            <span className="text-xs text-foreground truncate">{provider.label}</span>
            <span className="text-[11px] text-muted-foreground shrink-0">{kindLabel}</span>
          </div>
          <span className="text-[11px] text-muted-foreground truncate">
            {baseUrl}
            {needsKey && (hasKey ? " · Key stored" : " · No key")}
          </span>
        </div>
        <div className="flex-1" />
        <Button
          variant="ghost"
          size="xs"
          className="text-muted-foreground"
          onClick={() => setEditing(true)}
        >
          <PencilSimple size={13} /> Edit
        </Button>
        <Button
          variant="ghost"
          size="xs"
          className="text-muted-foreground hover:text-destructive"
          onClick={onRemove}
        >
          <Trash size={13} /> Remove
        </Button>
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-border bg-surface px-3 py-2.5 flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <Input
          className="h-7 w-52 text-xs"
          defaultValue={provider.label}
          onBlur={(e) => onChange({ label: e.target.value.trim() || (kindLabel ?? "Provider") })}
        />
        <span className="text-[11px] text-muted-foreground">{kindLabel}</span>
        <div className="flex-1" />
        <Button variant="ghost" size="xs" onClick={() => setEditing(false)}>
          <Check size={13} /> Done
        </Button>
        <Button
          variant="ghost"
          size="xs"
          className="text-muted-foreground hover:text-destructive"
          onClick={onRemove}
        >
          <Trash size={13} /> Remove
        </Button>
      </div>

      <div className="flex items-center gap-2">
        <span className="text-xs text-muted-foreground w-16 shrink-0">Base URL</span>
        <Input
          className="h-7 flex-1 text-xs"
          placeholder={DEFAULT_BASE_URL[provider.kind]}
          defaultValue={provider.baseUrl ? normalizeBaseUrl(provider.baseUrl) : ""}
          onBlur={(e) => {
            const url = normalizeBaseUrl(e.target.value);
            e.target.value = url;
            onChange({ baseUrl: url || null });
          }}
        />
      </div>

      {needsKey && (
        <div className="flex items-center gap-2">
          <span className="text-xs text-muted-foreground w-16 shrink-0">API key</span>
          {hasKey ? (
            <div className="flex items-center gap-1.5">
              <span className="text-xs text-foreground">Key stored</span>
              <Button
                variant="ghost"
                size="xs"
                className="text-destructive hover:text-destructive hover:bg-destructive/10"
                onClick={removeKey}
              >
                <X size={13} /> Remove
              </Button>
            </div>
          ) : (
            <>
              <Input
                type="password"
                className="h-7 flex-1 text-xs"
                placeholder="sk-…"
                value={keyDraft}
                onChange={(e) => setKeyDraft(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && saveKey()}
              />
              <Button size="xs" onClick={saveKey} disabled={!keyDraft.trim()}>
                Save
              </Button>
            </>
          )}
        </div>
      )}
    </div>
  );
}
