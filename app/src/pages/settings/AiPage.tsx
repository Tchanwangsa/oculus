import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { CircleNotch, PaperPlaneRight, Plus, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { FallbackList } from "@/components/llm/FallbackList";
import { ModelBrowser } from "@/components/llm/ModelBrowser";
import { ModelSelect } from "@/components/llm/ModelSelect";
import {
  DEFAULT_BASE_URL,
  normalizeBaseUrl,
  ProviderCard,
} from "@/components/llm/ProviderCard";
import {
  ADDABLE_PROVIDER_KINDS,
  getLlmSettings,
  modelKey,
  MAX_FALLBACKS,
  PROVIDER_KINDS,
  sameModel,
  setLlmSettings,
  type LlmProvider,
  type LlmProviderKind,
  type LlmSettings,
  type ModelRef,
} from "@/lib/db";
import { Section, StatRow } from "./section";

interface UsageSummary {
  monthPromptTokens: number;
  monthCompletionTokens: number;
  monthCostUsd: number;
  monthRequests: number;
}

/** Provider ids are keychain accounts, so they are readable and stable:
 *  `openrouter`, then `openrouter-2` for a second account of the same kind. */
function newProviderId(kind: string, existing: LlmProvider[]): string {
  if (!existing.some((p) => p.id === kind)) return kind;
  for (let n = 2; ; n++) {
    const id = `${kind}-${n}`;
    if (!existing.some((p) => p.id === id)) return id;
  }
}

export default function SettingsAiPage() {
  const [settings, setSettings] = useState<LlmSettings | null>(null);
  const [usage, setUsage] = useState<UsageSummary | null>(null);
  const [browsing, setBrowsing] = useState(false);
  const [adding, setAdding] = useState(false);
  const [testModel, setTestModel] = useState<ModelRef | null>(null);
  const [testPrompt, setTestPrompt] = useState("");
  const [testOutput, setTestOutput] = useState<string | null>(null);
  const [testError, setTestError] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);

  /** Latest saved settings, tracked outside render: ticking several models in
   *  the browser fires edits faster than React re-renders, and a patch built
   *  from a stale copy would silently drop the previous one. */
  const latest = useRef<LlmSettings | null>(null);

  /** Merge a patch into settings and persist — settings save as you go. The
   *  function form sees whatever the last edit wrote. */
  const update = useCallback(
    async (
      patch: Partial<LlmSettings> | ((prev: LlmSettings) => Partial<LlmSettings>),
    ) => {
      const prev = latest.current ?? (await getLlmSettings());
      const next = { ...prev, ...(typeof patch === "function" ? patch(prev) : patch) };
      latest.current = next;
      setSettings(next);
      await setLlmSettings(next);
    },
    [],
  );

  const refreshUsage = useCallback(() => {
    invoke<UsageSummary>("llm_usage_summary").then(setUsage).catch(() => {});
  }, []);

  useEffect(() => {
    getLlmSettings().then((s) => {
      latest.current = s;
      setSettings(s);
    });
    refreshUsage();
  }, [refreshUsage]);

  // Streamed deltas from the test prompt append to the output area.
  const testBuf = useRef("");
  useEffect(() => {
    const un = listen<string>("llm-test-delta", (e) => {
      testBuf.current += e.payload;
      setTestOutput(testBuf.current);
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  if (!settings) return null;

  const addProvider = async (draft: {
    kind: LlmProviderKind;
    label: string;
    baseUrl: string;
    key: string;
  }) => {
    const provider: LlmProvider = {
      id: newProviderId(draft.kind, settings.providers),
      kind: draft.kind,
      label: draft.label.trim() || (PROVIDER_KINDS.find((p) => p.kind === draft.kind)?.label ?? "Provider"),
      baseUrl: normalizeBaseUrl(draft.baseUrl) || null,
    };
    await update((prev) => ({ providers: [...prev.providers, provider] }));
    if (draft.key.trim()) {
      await invoke("llm_set_api_key", { providerId: provider.id, key: draft.key.trim() });
    }
    setAdding(false);
  };

  /** Removing a provider takes its models with it — a library entry pointing
   *  at an endpoint that no longer exists is a call that fails at send time. */
  const removeProvider = async (id: string) => {
    const keeps = (m: ModelRef) => m.providerId !== id;
    await update((prev) => ({
      providers: prev.providers.filter((p) => p.id !== id),
      library: prev.library.filter(keeps),
      chatModel: prev.chatModel && keeps(prev.chatModel) ? prev.chatModel : null,
      summaryModel: prev.summaryModel && keeps(prev.summaryModel) ? prev.summaryModel : null,
      fallbacks: prev.fallbacks.filter(keeps),
    }));
    await invoke("llm_delete_api_key", { providerId: id }).catch(() => {});
  };

  const toggleLibrary = (m: ModelRef) =>
    update((prev) => {
      if (!prev.library.some((l) => sameModel(l, m))) {
        return { library: [...prev.library, m] };
      }
      const keeps = (x: ModelRef) => !sameModel(x, m);
      return {
        library: prev.library.filter(keeps),
        chatModel: sameModel(prev.chatModel, m) ? null : prev.chatModel,
        summaryModel: sameModel(prev.summaryModel, m) ? null : prev.summaryModel,
        fallbacks: prev.fallbacks.filter(keeps),
      };
    });

  /** The chain is capped, so a new entry lands at the bottom and pushes the
   *  old bottom out — no dialog, no "which one do you want to drop". */
  const addFallback = (m: ModelRef) =>
    update((prev) => {
      const rest = prev.fallbacks.filter((f) => !sameModel(f, m));
      return { fallbacks: [...rest.slice(0, MAX_FALLBACKS - 1), m] };
    });

  const runTest = async () => {
    const prompt = testPrompt.trim();
    if (!prompt || testing) return;
    setTesting(true);
    setTestError(null);
    testBuf.current = "";
    setTestOutput("");
    try {
      await invoke("llm_test_prompt", { prompt, model: testModel ?? settings.chatModel });
    } catch (e) {
      setTestError(String(e));
    } finally {
      setTesting(false);
      refreshUsage();
    }
  };

  const providerLabel = (id: string) =>
    settings.providers.find((p) => p.id === id)?.label ?? "missing provider";

  return (
    <div className="flex flex-col gap-8">
      <Section
        title="Providers"
        description="Local and cloud providers share the same OpenAI-compatible API, and you can keep as many as you like configured at once. Keys are stored in the macOS keychain, never in the database."
      >
        <div className="flex flex-col gap-2 py-1">
          {settings.providers.map((p) => (
            <ProviderCard
              key={p.id}
              provider={p}
              onChange={(patch) =>
                update((prev) => ({
                  providers: prev.providers.map((x) =>
                    x.id === p.id ? { ...x, ...patch } : x,
                  ),
                }))
              }
              onRemove={() => removeProvider(p.id)}
            />
          ))}
          <div>
            <Button variant="outline" size="xs" onClick={() => setAdding(true)}>
              <Plus size={13} /> Add provider
            </Button>
          </div>
        </div>
      </Section>

      <Section
        title="Model library"
        description="The models you actually use, gathered from every provider. Browsing a provider lists everything it serves; the library is what the rest of the app chooses from."
      >
        <div className="flex items-center justify-between gap-4 py-2">
          <span className="text-xs text-muted-foreground">
            {settings.library.length} model{settings.library.length === 1 ? "" : "s"}
          </span>
          <Button
            variant="ghost"
            size="xs"
            onClick={() => setBrowsing(true)}
            disabled={settings.providers.length === 0}
          >
            <Plus size={13} /> Add models
          </Button>
        </div>

        {settings.library.length === 0 ? (
          <p className="text-xs text-muted-foreground py-1">
            {settings.providers.length === 0
              ? "Add a provider first, then pick models from it."
              : "Nothing yet — add models to choose one for chat."}
          </p>
        ) : (
          <div className="flex flex-col gap-1 py-1">
            {settings.library.map((m) => (
              <div
                key={modelKey(m)}
                className="flex items-center gap-2 rounded-md border border-border bg-surface px-2 py-1.5"
              >
                <span className="text-xs text-foreground truncate">{m.model}</span>
                <span className="text-[11px] text-muted-foreground truncate">
                  {providerLabel(m.providerId)}
                </span>
                <div className="flex-1" />
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label={`Remove ${m.model}`}
                  className="text-muted-foreground hover:text-destructive"
                  onClick={() => toggleLibrary(m)}
                >
                  <X size={12} />
                </Button>
              </div>
            ))}
          </div>
        )}
      </Section>

      <Section
        title="Defaults"
        description="Chat answers questions over your library; the summary model powers automations like sync digests. Either can be changed per conversation from the chat composer."
      >
        <div className="flex items-center justify-between gap-4 py-2">
          <span className="text-xs text-muted-foreground">Chat model</span>
          <ModelSelect
            library={settings.library}
            providers={settings.providers}
            value={settings.chatModel}
            onChange={(m) => update({ chatModel: m })}
            className="w-72"
          />
        </div>
        <div className="flex items-center justify-between gap-4 py-2">
          <span className="text-xs text-muted-foreground">Summary model</span>
          <ModelSelect
            library={settings.library}
            providers={settings.providers}
            value={settings.summaryModel}
            onChange={(m) => update({ summaryModel: m })}
            className="w-72"
          />
        </div>
      </Section>

      <Section
        title="Fallbacks"
        description={`Tried in order when the chosen model cannot run — a local model the machine has no room for, or a provider you removed. Up to ${MAX_FALLBACKS}; drag to reorder.`}
      >
        <FallbackList
          items={settings.fallbacks}
          providers={settings.providers}
          onReorder={(next) => update({ fallbacks: next })}
          onRemove={(m) =>
            update((prev) => ({
              fallbacks: prev.fallbacks.filter((f) => !sameModel(f, m)),
            }))
          }
        />
        <div className="flex items-center justify-between gap-4 py-2">
          <span className="text-xs text-muted-foreground">
            {settings.fallbacks.length >= MAX_FALLBACKS
              ? "Full — adding one drops the last in the list"
              : "Add a fallback"}
          </span>
          <ModelSelect
            library={settings.library}
            providers={settings.providers}
            value={null}
            onChange={addFallback}
            placeholder="Add a model"
            className="w-72"
          />
        </div>
        <p className="text-xs text-muted-foreground pt-1">
          A local model is only loaded when the machine has room for it — memory the
          system could actually give it, with anything Ollama already holds loaded
          counted as available. If it doesn't fit, the first fallback that does runs
          instead; with none set, the request is refused rather than risking the
          machine.
        </p>
      </Section>

      <Section
        title="Limits"
        description="Calls are refused once a monthly cap is reached. Leave blank for no cap; local models cost nothing."
      >
        <div className="flex items-center justify-between gap-4 py-2">
          <Label htmlFor="llm-cap-usd" className="text-xs font-normal text-muted-foreground">
            Monthly spend (USD)
          </Label>
          <Input
            id="llm-cap-usd"
            type="number"
            min={0}
            step="0.5"
            className="h-7 w-28 text-xs text-right"
            defaultValue={settings.limits.monthlyUsd ?? ""}
            onBlur={(e) =>
              update({
                limits: {
                  ...settings.limits,
                  monthlyUsd: e.target.value === "" ? null : Number(e.target.value),
                },
              })
            }
          />
        </div>
        <div className="flex items-center justify-between gap-4 py-2">
          <Label htmlFor="llm-cap-tokens" className="text-xs font-normal text-muted-foreground">
            Monthly tokens
          </Label>
          <Input
            id="llm-cap-tokens"
            type="number"
            min={0}
            step={100000}
            className="h-7 w-28 text-xs text-right"
            defaultValue={settings.limits.monthlyTokens ?? ""}
            onBlur={(e) =>
              update({
                limits: {
                  ...settings.limits,
                  monthlyTokens: e.target.value === "" ? null : Number(e.target.value),
                },
              })
            }
          />
        </div>
      </Section>

      <Section title="Usage this month">
        <StatRow label="Requests" value={String(usage?.monthRequests ?? 0)} />
        <StatRow
          label="Tokens"
          value={`${(
            (usage?.monthPromptTokens ?? 0) + (usage?.monthCompletionTokens ?? 0)
          ).toLocaleString()} (${(usage?.monthPromptTokens ?? 0).toLocaleString()} in / ${(
            usage?.monthCompletionTokens ?? 0
          ).toLocaleString()} out)`}
        />
        <StatRow label="Cost" value={`$${(usage?.monthCostUsd ?? 0).toFixed(2)}`} />
      </Section>

      <Section
        title="Test"
        description="Sends one prompt straight to a model — the way to check a provider's key and base URL before anything depends on it."
      >
        <div className="flex items-center justify-between gap-4 py-2">
          <span className="text-xs text-muted-foreground">Model</span>
          <ModelSelect
            library={settings.library}
            providers={settings.providers}
            value={testModel ?? settings.chatModel}
            onChange={setTestModel}
            className="w-72"
          />
        </div>
        <div className="flex items-center gap-1.5 py-2">
          <Input
            className="h-7 flex-1 text-xs"
            placeholder="Say hello in five words"
            value={testPrompt}
            onChange={(e) => setTestPrompt(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && runTest()}
            disabled={testing}
          />
          <Button size="xs" onClick={runTest} disabled={testing || !testPrompt.trim()}>
            {testing ? (
              <CircleNotch size={13} className="animate-spin" />
            ) : (
              <PaperPlaneRight size={13} />
            )}
            Run
          </Button>
        </div>
        {testError && <p className="text-xs text-destructive py-1">{testError}</p>}
        {testOutput !== null && !testError && (
          <p className="text-xs text-foreground whitespace-pre-wrap py-1">
            {testOutput || (testing ? "…" : "")}
          </p>
        )}
      </Section>

      <ModelBrowser
        open={browsing}
        onOpenChange={setBrowsing}
        providers={settings.providers}
        library={settings.library}
        onToggle={toggleLibrary}
      />
      <AddProviderDialog open={adding} onOpenChange={setAdding} onAdd={addProvider} />
    </div>
  );
}

function AddProviderDialog({
  open,
  onOpenChange,
  onAdd,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAdd: (draft: {
    kind: LlmProviderKind;
    label: string;
    baseUrl: string;
    key: string;
  }) => void;
}) {
  const [kind, setKind] = useState<LlmProviderKind>(ADDABLE_PROVIDER_KINDS[0].kind);
  const [label, setLabel] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [key, setKey] = useState("");
  const needsKey = PROVIDER_KINDS.find((p) => p.kind === kind)?.needsKey ?? true;
  const kindLabel = PROVIDER_KINDS.find((p) => p.kind === kind)?.label ?? "";

  // Each opening starts clean; the previous draft was either added or dropped.
  useEffect(() => {
    if (open) {
      setKind(ADDABLE_PROVIDER_KINDS[0].kind);
      setLabel("");
      setBaseUrl("");
      setKey("");
    }
  }, [open]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Add provider</DialogTitle>
          <DialogDescription>
            An endpoint speaking the OpenAI-compatible API. Leave the base URL blank to
            use the default for its kind.
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-2.5">
          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground w-16 shrink-0">Kind</span>
            <Select value={kind} onValueChange={(v) => setKind(v as LlmProviderKind)}>
              <SelectTrigger size="sm" className="h-7 flex-1 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {ADDABLE_PROVIDER_KINDS.map((p) => (
                  <SelectItem key={p.kind} value={p.kind} className="text-xs">
                    {p.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground w-16 shrink-0">Name</span>
            <Input
              className="h-7 flex-1 text-xs"
              placeholder={kindLabel}
              value={label}
              onChange={(e) => setLabel(e.target.value)}
            />
          </div>

          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground w-16 shrink-0">Base URL</span>
            <Input
              className="h-7 flex-1 text-xs"
              placeholder={DEFAULT_BASE_URL[kind]}
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
            />
          </div>

          {needsKey && (
            <div className="flex items-center gap-2">
              <span className="text-xs text-muted-foreground w-16 shrink-0">API key</span>
              <Input
                type="password"
                className="h-7 flex-1 text-xs"
                placeholder="sk-…"
                value={key}
                onChange={(e) => setKey(e.target.value)}
              />
            </div>
          )}
        </div>

        <DialogFooter>
          <Button
            size="sm"
            onClick={() => onAdd({ kind, label, baseUrl, key })}
            disabled={kind === "custom" && !baseUrl.trim()}
          >
            Add provider
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
