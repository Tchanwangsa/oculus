import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { listVoiceModels, type VoiceSettings } from "@/lib/studio/voice";
import { blurOnEnter, SettingRow } from "./SettingRow";
import type { SaveVoice, VoiceErrors } from "./NarrationVoiceSection";

type TextField = "baseUrl" | "model" | "voice";

/**
 * The server engine's fields: Server URL, Model (a select once "List
 * models" has the server's ids) and Voice, typed since servers can't list
 * theirs. A text field saves on blur or Enter, and shows what was stored.
 */
export function ServerFields({ settings, errors, save, clearError }: {
  settings: VoiceSettings;
  errors: VoiceErrors;
  save: SaveVoice;
  clearError: (field: TextField) => void;
}) {
  const [urlDraft, setUrlDraft] = useState(settings.baseUrl ?? "");
  const [modelDraft, setModelDraft] = useState(settings.model ?? "");
  const [voiceDraft, setVoiceDraft] = useState(settings.voice ?? "");
  const [models, setModels] = useState<string[] | null>(null);
  const [listing, setListing] = useState(false);
  const [listError, setListError] = useState<string | null>(null);

  const drafts: Record<TextField, (value: string) => void> = {
    baseUrl: setUrlDraft,
    model: setModelDraft,
    voice: setVoiceDraft,
  };

  /** Save a text field when its draft differs from what is stored. */
  const commit = async (field: TextField, draft: string) => {
    const value = draft.trim() || null;
    if (value === (settings[field] ?? null)) {
      clearError(field);
      return;
    }
    if (field === "baseUrl") {
      // Another server's list says nothing about this one.
      setModels(null);
      setListError(null);
    }
    const stored = await save(field, { [field]: value });
    // Show the stored form (trimmed, normalised) of the field just saved.
    if (stored) drafts[field](stored[field] ?? "");
  };

  // The field as typed: Rust normalises it, and a bad one fails here too.
  const list = async () => {
    const base = urlDraft.trim();
    if (!base) return;
    setListing(true);
    setListError(null);
    try {
      setModels(await listVoiceModels(base));
    } catch (cause) {
      setModels(null);
      setListError(String(cause));
    } finally {
      setListing(false);
    }
  };

  // The stored model stays choosable even when the server does not list it.
  const options =
    models && models.length > 0
      ? settings.model && !models.includes(settings.model)
        ? [settings.model, ...models]
        : models
      : null;

  return (
    <>
      <SettingRow
        label="Server URL"
        hint="Where the server answers, with or without /v1."
        error={errors.baseUrl}
      >
        <Input
          aria-label="TTS server URL"
          type="url"
          autoComplete="off"
          spellCheck={false}
          value={urlDraft}
          onChange={(e) => setUrlDraft(e.target.value)}
          onBlur={() => void commit("baseUrl", urlDraft)}
          onKeyDown={blurOnEnter}
          placeholder="http://127.0.0.1:8880"
          className="h-7 w-80 text-xs"
        />
      </SettingRow>

      <SettingRow
        label="Model"
        hint={
          options
            ? "The models the server lists."
            : "The server's model id, e.g. kokoro. List models asks the server for its ids."
        }
        error={errors.model ?? listError}
        note={models?.length === 0 ? "The server listed no models: type the model id." : null}
      >
        <div className="flex w-80 items-center gap-2">
          {options ? (
            <Select
              value={settings.model ?? ""}
              onValueChange={(model) => {
                setModelDraft(model);
                void save("model", { model });
              }}
            >
              <SelectTrigger aria-label="TTS model" size="sm" className="h-7 min-w-0 flex-1 text-xs">
                <SelectValue placeholder="Choose a model" />
              </SelectTrigger>
              <SelectContent>
                {options.map((id) => (
                  <SelectItem key={id} value={id} className="text-xs">
                    {id}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          ) : (
            <Input
              aria-label="TTS model"
              autoComplete="off"
              spellCheck={false}
              value={modelDraft}
              onChange={(e) => setModelDraft(e.target.value)}
              onBlur={() => void commit("model", modelDraft)}
              onKeyDown={blurOnEnter}
              placeholder="kokoro"
              className="h-7 min-w-0 flex-1 text-xs"
            />
          )}
          <Button
            variant="outline"
            size="xs"
            className="w-[5.5rem] shrink-0"
            disabled={!urlDraft.trim() || listing}
            onClick={() => void list()}
          >
            {listing ? "Listing…" : "List models"}
          </Button>
        </div>
      </SettingRow>

      <SettingRow
        label="Voice"
        hint="The server's voice name, e.g. af_heart. Servers can't list their voices, so type it."
        error={errors.voice}
      >
        <Input
          aria-label="TTS voice"
          autoComplete="off"
          spellCheck={false}
          value={voiceDraft}
          onChange={(e) => setVoiceDraft(e.target.value)}
          onBlur={() => void commit("voice", voiceDraft)}
          onKeyDown={blurOnEnter}
          placeholder="af_heart"
          className="h-7 w-80 text-xs"
        />
      </SettingRow>
    </>
  );
}
