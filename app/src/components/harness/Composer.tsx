import { useEffect, useRef, useState } from "react";
import { PaperPlaneTilt, Stop, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { MentionInput, type MentionInputHandle } from "@/components/harness/MentionInput";
import { MentionMenu } from "@/components/harness/MentionMenu";
import { useMentionMenu } from "@/components/harness/useMentionMenu";
import { ModelPicker } from "@/components/harness/ModelPicker";
import { UsageMeter } from "@/components/harness/UsageMeter";
import { SubjectSelect } from "@/components/harness/SubjectSelect";
import { type Subject } from "@/lib/db";
import {
  defaultSelection,
  providerLabel,
  type Provider,
  type RateWindow,
  type ThreadUsage,
} from "@/lib/harness";
import { useProviderModels } from "@/hooks/useProviderModels";
import { useFileDrop } from "@/hooks/useFileDrop";
import { ImageLightbox } from "@/components/ui/Lightbox";
import {
  imagePaths,
  pendingFromFile,
  pendingFromPath,
  releaseAttachment,
  writeAttachment,
  type PendingAttachment,
} from "@/lib/attachments";
import { signInState, useSignInStatus } from "@/hooks/useSignInStatus";
import { SignInDialog, useSignIn } from "@/components/harness/SignInDialog";
import { cn } from "@/lib/utils";

/**
 * One box: the text, then a row carrying the model picker on the left and the
 * usage wheel and send/stop button on the right. Enter sends, Shift+Enter
 * breaks a line — bb's keys.
 *
 * While a turn runs the box still takes a message: it is *queued* rather than
 * sent (Rust holds it — `Queue` in `app/src-tauri/src/harness/mod.rs`) and it
 * appears as a pending bubble at the end of the thread. Stop then keeps its
 * word — nothing more goes out — and hands back what was waiting, which
 * arrives here as `restore` and lands in the box rather than being lost.
 * So while a turn runs there are two buttons and not one: stop is always
 * reachable, and send appears beside it as soon as there is something to
 * queue.
 *
 * The row's controls are all one height (24px). A send button larger than the
 * picker beside it reads as the box's subject rather than its verb.
 *
 * Scope sits *above* the box rather than in the control row, and only while
 * the thread is new: it is a choice made once, before the first message, and
 * an open thread cannot change it. Showing a dead control under every message
 * for the rest of the thread spends the row on nothing.
 *
 * `@` opens a file menu, narrowed to the thread's subject. Picking a file
 * writes its **library path** into the message — nothing is read here and no
 * content is attached. The agent has the library in front of it and its own
 * tools for opening a file; a path is all it was ever missing, and one it can
 * hand straight to `oculus read`. The box *draws* that path as a chip
 * carrying the file's own glyph and display name (`MentionInput.tsx` over the
 * shared `app/src/components/markdown/FileChip.tsx`, which the thread's own
 * bubbles use too), which is why the text here is not the box's value but its
 * serialization: what is read out of the editor, checked for emptiness and
 * handed to `onSend` is always the path form.
 *
 * The `@` machinery itself is no longer this file's: the token parser, the
 * lookup, the list and its keys live in `./useMentionMenu.ts` and
 * `./MentionMenu.tsx`, because a task body wants the same mentions
 * (`app/src/pages/TaskPage.tsx`). What stays here is the only part that was
 * ever the composer's — the scope it narrows to, and the Enter that sends
 * when the menu is not the one claiming it.
 */
export function Composer({
  provider,
  model,
  reasoning,
  providerLocked,
  subjects,
  subjectId,
  onSubject,
  subjectLocked,
  running,
  usage,
  rateLimits,
  restore,
  onRestored,
  onProvider,
  onModel,
  onReasoning,
  onSend,
  onStop,
  autoFocus,
}: {
  provider: Provider;
  model: string | null;
  /** Reasoning effort for the next turn; null leaves the flag off. */
  reasoning: string | null;
  /** An open thread keeps its provider; only a new one can pick. */
  providerLocked: boolean;
  subjects: Subject[];
  /** null is the general thread — the whole library. */
  subjectId: number | null;
  onSubject: (id: number | null) => void;
  /** An open thread keeps its scope too, for the same reason. */
  subjectLocked: boolean;
  running: boolean;
  usage: ThreadUsage | null;
  rateLimits: RateWindow[];
  /** Messages stop dropped out of the queue, handed back to be typed over.
   *  The counter is what is watched: the same text twice is two restores. */
  restore?: { text: string; n: number } | null;
  onRestored?: () => void;
  onProvider: (p: Provider) => void;
  onModel: (m: string | null) => void;
  onReasoning: (level: string | null) => void;
  onSend: (text: string) => void;
  onStop: () => void;
  autoFocus?: boolean;
}) {
  /** The message the box would send: chips already back in their path form.
   *  Every emptiness check below reads this, not the editor. */
  const [text, setText] = useState("");
  /**
   * Pictures pasted or dropped in, still only in memory.
   *
   * They are written to the library on send and not before (`writeAttachment`,
   * `app/src/lib/attachments.ts`), so a screenshot pasted and then removed
   * leaves nothing on disk. What goes out is their paths, appended to the
   * message — the agent opens them itself, exactly as it opens a mention.
   */
  const [attached, setAttached] = useState<PendingAttachment[]>([]);
  /** Why a picture could not be attached or written — a drop that was not an
   *  image, or a refusal from Rust. Never a reason to lose the message. */
  const [attachError, setAttachError] = useState<string | null>(null);
  /** True while the pictures for a send are being written. */
  const [writing, setWriting] = useState(false);
  const ref = useRef<MentionInputHandle>(null);
  /** The `@` menu, scoped to this thread's subject — `null` being the general
   *  thread, which is the whole library. It positions itself at the caret and
   *  needs nothing from this file to do it. */
  const mentions = useMentionMenu({ subjectId, input: ref });
  /** This whole box, for the file drop below: a drag is aimed at a surface,
   *  and the surface is the box and its strip of attachments rather than the
   *  editor's text. It is no longer the `@` menu's anchor — that is the caret
   *  now — so it is this file's own ref. */
  const wrapRef = useRef<HTMLDivElement>(null);

  // What stop gave back. It is put *before* anything already typed, because
  // it was typed first, and the box is focused with the caret at the end of
  // it so the next key carries on where the student left off. Its mentions
  // come back as chips rather than as the paths they were sent as — a
  // restored message should read the way it read when it was typed.
  useEffect(() => {
    if (!restore) return;
    ref.current?.prepend(restore.text);
    onRestored?.();
  }, [restore, onRestored]);

  // Only the provider the picker is on is worth asking a CLI about: a
  // composer that is on Claude should not spawn the others for lists nobody
  // has opened the menu to see.
  const { providers: pickerProviders } = useProviderModels(provider);

  /**
   * The warning *before* the failure: the agent this box would send to has no
   * credentials, said here rather than found out as a red row after sending.
   *
   * Three states, the discipline `providerHealth` already follows for the same
   * reason: `unknown` — the probe has not landed, or this is opencode, whose
   * store is per provider — draws **nothing at all**, because a line that
   * flashed the wrong answer for a beat would be worse than no line. Only a
   * measured `out` says so.
   *
   * And it never disables sending. The status is a cached read of a CLI's own
   * store, which a student can change in a terminal without this app hearing
   * about it; locking the box on a stale read would be the app refusing to do
   * the one thing it is for.
   */
  const { statuses, recheck } = useSignInStatus();
  const signedOut = signInState(statuses, provider) === "out";
  const [signInOpen, setSignInOpen] = useState(false);
  const signIn = useSignIn(recheck);

  // No turn goes out without a model and a level, so an empty selection — a
  // fetched catalogue before its CLI has answered — is filled the moment a
  // list exists.
  const active = pickerProviders.find((p) => p.id === provider);
  useEffect(() => {
    if (model || !active || active.loading || active.models.length === 0) return;
    const pick = defaultSelection(active.models);
    if (!pick.model) return;
    onModel(pick.model);
    onReasoning(pick.reasoning);
  }, [model, active, onModel, onReasoning]);

  /** Whether there is a message at all: words, pictures, or both. */
  const ready = text.trim().length > 0 || attached.length > 0;

  /** Pictures from a paste, which arrive as `File`s the clipboard owns. */
  function attach(files: File[]) {
    if (!files.length) return;
    setAttachError(null);
    setAttached((a) => [...a, ...files.map(pendingFromFile)]);
  }

  /**
   * …and from a drop, which arrives as paths (`useFileDrop`).
   *
   * A drop of something that is not an image says so rather than being
   * ignored: a dropped PDF is a reasonable thing to try, and silence would
   * read as a broken drop target. The course files it *would* mean are already
   * reachable through `@`.
   */
  function attachPaths(paths: string[]) {
    const pictures = imagePaths(paths);
    if (!pictures.length) {
      if (paths.length) setAttachError("Only images can be attached — use @ for a course file.");
      return;
    }
    setAttachError(null);
    setAttached((a) => [...a, ...pictures.map(pendingFromPath)]);
  }

  function detach(id: string) {
    setAttached((a) => {
      const gone = a.find((x) => x.id === id);
      if (gone) releaseAttachment(gone);
      return a.filter((x) => x.id !== id);
    });
  }

  const dropping = useFileDrop(wrapRef, attachPaths);
  /** The attached picture being looked at, as the src the chip already drew.
   *  A chip is 56px of a screenshot, which is enough to tell two apart and
   *  not enough to check one — so it opens the same viewer the thread's cards
   *  do (`ImageLightbox`). */
  const [shown, setShown] = useState<string | null>(null);

  // Blob URLs outlive the component unless they are let go of. The list is
  // read through a ref so the cleanup runs once, on unmount, rather than on
  // every change to it.
  const attachedRef = useRef(attached);
  attachedRef.current = attached;
  useEffect(() => () => attachedRef.current.forEach(releaseAttachment), []);

  /**
   * Send, or queue.
   *
   * Pictures are written first and the message is only assembled once they
   * are on disk: a path in a message that points at nothing is worse than a
   * send that did not happen, so a refusal keeps the box exactly as it was
   * and says why.
   */
  const send = async () => {
    const t = text.trim();
    const pictures = attached;
    if ((!t && !pictures.length) || writing) return;

    let paths: string[] = [];
    if (pictures.length) {
      setWriting(true);
      try {
        paths = await Promise.all(pictures.map(writeAttachment));
      } catch (e) {
        setAttachError(String(e));
        return;
      } finally {
        setWriting(false);
      }
      pictures.forEach(releaseAttachment);
      setAttached([]);
    }

    ref.current?.clear();
    mentions.close();
    setAttachError(null);
    // The paths go on their own line under what was typed, fenced the way a
    // mention is, so the bubble draws them and the agent reads them with the
    // one matcher both already use (`splitLibraryPaths`).
    const body = [t, paths.map((p) => `\`${p}\``).join(" ")].filter(Boolean).join("\n\n");
    // Whether this goes out now or waits behind the running turn is Rust's
    // call, not this box's: it owns the queue and the order.
    onSend(body);
  };

  return (
    <div ref={wrapRef} className="flex flex-col gap-1.5">
      <ImageLightbox
        src={shown ?? ""}
        alt={attached.find((a) => a.preview === shown)?.name}
        open={shown !== null}
        onOpenChange={(o) => !o && setShown(null)}
      />
      {!subjectLocked && (
        <div className="flex items-center px-0.5">
          <SubjectSelect
            subjects={subjects}
            value={subjectId}
            onChange={onSubject}
            className="h-6 max-w-[200px] rounded-full border-border/70 bg-card px-2 text-[11px] text-muted-foreground shadow-none hover:bg-accent hover:text-foreground"
          />
        </div>
      )}

      {/* Where this sits in the tree no longer decides where it draws: it
          portals out and hangs off the caret's own rect. It stays here
          because this is the box it belongs to. */}
      <MentionMenu {...mentions.menu} />

      {attachError && (
        <div className="px-0.5 text-[11px] text-destructive">{attachError}</div>
      )}

      {signedOut && (
        <div className="flex items-center gap-1.5 px-0.5 text-[11px] text-muted-foreground">
          <span>{providerLabel(provider)} is signed out.</span>
          <button
            type="button"
            onClick={() => setSignInOpen(true)}
            className="cursor-pointer text-brand underline-offset-2 hover:underline"
          >
            Sign in
          </button>
        </div>
      )}

      {signInOpen && (
        <SignInDialog
          provider={provider}
          run={signIn.run?.provider === provider ? signIn.run : null}
          onStart={() => signIn.start(provider)}
          onCode={(code) => signIn.submitCode(code)}
          onCancel={() => signIn.cancel()}
          onClose={() => {
            if (signIn.run?.result) signIn.clear();
            setSignInOpen(false);
          }}
        />
      )}

      <div
        className={cn(
          "flex flex-col gap-4 rounded-xl border border-border bg-card px-3 py-3 shadow-sm transition-[border-color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25",
          // A drag over the box says so on the box itself, in the same
          // vocabulary focus uses — there is nowhere else for a drop target
          // to be announced without a panel this app does not have.
          dropping && "border-brand ring-[3px] ring-brand/25",
        )}
      >
        <div className="flex flex-col gap-2">
          {attached.length > 0 && (
            <div className="flex flex-wrap items-center gap-2">
              {attached.map((a) => (
                <div key={a.id} className="group/att relative">
                  <button
                    type="button"
                    onClick={() => setShown(a.preview)}
                    aria-label={`Open ${a.name}`}
                    title={a.name}
                    className="block cursor-pointer overflow-hidden rounded-lg border border-border transition-colors hover:border-ring"
                  >
                    {/* `object-cover` here and `object-contain` in the thread,
                        and that is not an inconsistency: a chip this small is
                        an identifier, where a crop that fills the square tells
                        two screenshots apart better than a letterboxed
                        thumbnail two thirds of which is ground. The full
                        picture is one click away. */}
                    <img
                      src={a.preview}
                      alt={a.name}
                      className="block h-14 w-14 object-cover"
                    />
                  </button>
                  <button
                    type="button"
                    aria-label={`Remove ${a.name}`}
                    onClick={() => detach(a.id)}
                    className="absolute -right-1.5 -top-1.5 flex h-[18px] w-[18px] cursor-pointer items-center justify-center rounded-full border border-border bg-card text-muted-foreground opacity-0 transition-opacity group-hover/att:opacity-100 hover:text-foreground focus-visible:opacity-100"
                  >
                    <X size={9} weight="bold" />
                  </button>
                </div>
              ))}
            </div>
          )}
          <MentionInput
            ref={ref}
            autoFocus={autoFocus}
            onEdit={(next, caret) => {
              setText(next);
              mentions.track(next, caret);
            }}
            onFiles={attach}
            onBlur={mentions.close}
            onKeyDown={(e) => {
              // The menu's keys first, and only its own: anything it claims
              // comes back prevented, so Enter-sends below never fires on the
              // keystroke that picked a file.
              mentions.keyDown(e);
              if (e.defaultPrevented) return;
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                void send();
              }
            }}
            placeholder={
              running ? "Working… your next message waits its turn" : "What would you like to work on?"
            }
          />
        </div>
        <div className="flex items-center gap-1">
          <ModelPicker
            className="-ml-1.5"
            providers={pickerProviders}
            provider={provider}
            providerLocked={providerLocked}
            model={model}
            reasoning={reasoning}
            onProvider={onProvider}
            onModel={onModel}
            onReasoning={onReasoning}
          />
          <div className="flex-1" />
          <UsageMeter usage={usage} rateLimits={rateLimits} />
          {running && (
            <Button size="icon-xs" variant="ghost" className="shrink-0" aria-label="Stop" onClick={onStop}>
              <Stop weight="fill" />
            </Button>
          )}
          {(!running || ready) && (
            <Button
              size="icon-xs"
              disabled={!ready || writing}
              onClick={() => void send()}
              className="shrink-0"
              aria-label={running ? "Queue" : "Send"}
            >
              <PaperPlaneTilt />
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
