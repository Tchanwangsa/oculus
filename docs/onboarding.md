# Onboarding

First-run setup: a full-window walk through connecting Canvas, the library's
keys and an agent, shown in place of the shell until it is finished or
skipped through.

## Where

| Piece | Location |
| --- | --- |
| The gate, and the shell it replaces | `app/src/App.tsx`, `app/src/hooks/shell/useOnboardingGate.ts` |
| The gate's decision (pure, tested) | `app/src/lib/onboarding/gate.ts`, `app/tests/lib/onboarding/gate.test.ts` |
| Step ids, in order | `app/src/lib/onboarding/steps.ts` |
| The `onboarding` settings row | `app/src/lib/db/onboarding.ts` |
| Open or not, reopen, finish | `app/src/stores/shell/onboardingStore.ts` |
| The full-window view and the step order | `app/src/components/onboarding/Onboarding.tsx` |
| The step interface and the shared frame | `app/src/components/onboarding/types.ts`, `app/src/components/onboarding/StepFrame.tsx`, `app/src/components/onboarding/stepNav.ts` |
| The backdrop, progress rail and step names/glyphs | `app/src/components/onboarding/Backdrop.tsx`, `app/src/components/onboarding/StepRail.tsx`, `app/src/components/onboarding/stepMeta.ts` |
| Pieces step bodies share | `app/src/components/onboarding/OptionCard.tsx`, `app/src/components/onboarding/SetupOverview.tsx`, `app/src/components/onboarding/HeroHalo.tsx` |
| One file per step | `app/src/components/onboarding/WelcomeStep.tsx`, `app/src/components/onboarding/CanvasStep.tsx`, `app/src/components/onboarding/LibraryStep.tsx`, `app/src/components/onboarding/AgentStep.tsx`, `app/src/components/onboarding/DoneStep.tsx` |
| The Canvas step's card, its status and the automated sign-in section | `app/src/components/onboarding/canvas/`; what it shares with Settings, the credential form `app/src/components/settings/web/AutoSignIn.tsx` and the address field's state `app/src/components/settings/web/useCanvasAddressForm.ts` |
| The Library step's key cards, subject checklist and first sync | `app/src/components/onboarding/library/`; the key fields it shares with Settings, `app/src/hooks/keys/` |
| The Agent step's cards; prices, links and order (ordering tested) | `app/src/components/onboarding/agent/`, `app/src/lib/onboarding/agentOptions.ts`, `app/tests/lib/onboarding/agentOptions.test.ts` |
| Settings → Canvas → Setup: *Run setup again* | `app/src/components/settings/web/SetupSection.tsx` |

## Onboarding replaces the shell, not a tab

`App` renders `Onboarding` instead of `AppLayout` while it is open, so the
persisted tab strip (`oculus-tabs`) is never touched and no tab route exists
for it. `EventBridge` stays mounted either way: backend events, the quality
sweep and the index queue run during setup. While the gate decides, `App`
paints a blank `bg-background` window, so neither onboarding nor a
"not connected" state flashes. `App` applies the stored page zoom at boot
(`app/src/lib/ui/pageZoom.ts`), so setup renders at the shell's size; the
zoom shortcuts live in `AppLayout` and don't work during setup. Onboarding
owns its 44px drag header while the shell is unmounted. On Windows it reuses
`app/src/components/tabs/WindowControls.tsx` for minimize, maximize/restore and
close; native fullscreen hides the caption controls just as the shell does.

## The settings row decides, and existing setups never see it

One `settings` row, `onboarding`, JSON `{ completedAt, skipped }`, where
`skipped` holds step ids. At launch, once per window load:

1. `completedAt` set → the shell.
2. Row present with `completedAt: null` → onboarding (Settings asked for it).
3. Row absent, or malformed → the existing-setup probe. Canvas authenticated
   (`get_auth_status`, not `useAuth`, which starts at "disconnected") and at
   least one agent ready → write `completedAt` silently and show the shell.
   Otherwise → onboarding.

An agent is ready when supported on the current platform, installed and
signed in. opencode's sign-in can't be asked from outside its own page, so installed is enough for it, and the gate
never starts opencode ([harness.md](./harness.md)). Native Claude does not
count as ready on Windows, even when installed and signed in. The probe and
Agent step read the shared capabilities in `app/src/lib/harness/availability.ts`;
the step names supported agents and directs terminal-only setup to a terminal.
The probe asks Canvas first and skips the agents when it is not signed in, because the sign-in
check spawns every CLI. It reads through `loadBridgeHealth` and
`loadSignInStatus`, filling the same caches `useBridgeHealth` and
`useSignInStatus` read, so the shell's pickers don't ask again. A gate that
fails shows the shell.

## Steps share one interface and one frame

The order is `welcome`, `canvas`, `library`, `agent`, `done`. Every step takes
`StepProps` (`types.ts`): `{ onNext, onSkip }`. A step renders only its body
inside `StepFrame`, which draws the header, the body and a footer band with
Back / Skip for now / Next; the step passes `nextDisabled` to decide when Next
is enabled. Back and the skipped list come from the shell through
`stepNav.ts`, not props.

`Onboarding` draws a `max-w-2xl` card over `Backdrop` (a dot grid fading out
toward the window's edges under a soft indigo glow, both mixed from the theme
tokens, so dark mode needs no second set). On the three setup steps
`StepRail` heads the card: Canvas, Library and Agent joined by a line, each a
check once passed, a dash if skipped, its glyph when current or ahead. Each
step fades in once on arrival, not under `prefers-reduced-motion`.

`StepFrame` takes either an `icon` (a Phosphor glyph, drawn in an indigo tile
beside the title — the setup steps pass theirs from `stepMeta.ts`) or a `hero`
(art centred above a larger title, ringed by `HeroHalo`). The bookends use
`hero`: Welcome shows the Oculus mark (`app/public/oculus-mark.svg`), tiles
previewing the three steps (`SetupOverview`) and the four agents as
`ProviderMark` chips; Done shows a check and the same tiles marked set up or
skipped.

A step body lists its choices — agents, parse engines, keys — as
`OptionCard`s: a logo tile, a title with a status chip, a muted detail line,
an actions slot, and a `selected` state; with `onSelect` the card is a radio.
Its props are documented in the file.

Skip adds the step to `skipped`; Next removes it. Next on Done writes
`completedAt` and the skipped list, and the shell returns.

## The Canvas step: the address, one sign-in, and an optional way to never repeat it

- **One card** for the student's Canvas. Without an address it holds the
  address field Settings → Canvas uses (`useCanvasAddressForm`): Rust checks
  the address is Canvas and detects its sign-in
  ([auth.md](./auth.md#one-institution-profile-says-which-university)), and a
  refusal shows under the field. With one, the card names the host, says how
  it signs in until connected, and offers *Change* unless a synced library
  pins the address.
  Its chip comes from `useAuth` through `useCanvasConnection`: *Connected* (the
  card selected), *Signing in…* with a line pointing at the Canvas window,
  *Session expired*, or *Not connected*. `useAuth` starts at "disconnected"
  before `get_auth_status` answers, so the hook asks once more itself and
  shows no chip until one of them settles it. *Connect* (or *Sign in again*,
  or *Reconnect* once connected) is `useAuth().connect()`: the saved
  credentials first, then the login window; the finish arrives as
  `canvas-auth-*` events ([auth.md](./auth.md#every-sign-in-ends-in-one-place)).
- **Never sign in again** shows only at a university that signs in through
  Okta, the one sign-in it answers. Folded by default, it says what the automated
  sign-in does, that push-only (Okta Verify) accounts can't use it, and how
  to get the authenticator key by re-enrolling: *Okta settings* opens
  `<sso host>/enduser/settings` with `openUrl`, then add Google
  Authenticator, *Can't scan?*, scan the QR with the phone too, paste the key
  or the whole `otpauth://` link. Under the guide sits Settings' own
  `AutoSignIn` with `embedded`, which drops its title and guide and keeps the
  fields open until credentials are saved. Its *Save and test* signs in, so
  it connects the card too.
- **Next is enabled once Canvas is connected.** Skip is always there.

## The Library step: two keys, the subjects, then the first sync

- **MinerU is the recommended card**: free for 5,000 files a day
  (`DAILY_FILES` in `parse/mineru/ledger.rs`), with *Get a key* opening
  MinerU's token page (`mineru.net/apiManage/token`) and a paste field.
  MinerU tokens last 90 days; an expired one surfaces as the parse latch's
  *Expired* state in Settings → Parsing. **Voyage is a quieter dashed row**,
  optional: without it the agent still searches the library by text. Both
  fields are Settings' own (`useMineruKey`, `useVoyageKey` on `useCloudKey`):
  Rust checks a key before storing it, and the notes — Rust's refusal, or
  *Saved, but … was unreachable* — read the same. A MinerU key saved here
  re-queues PDFs that failed for want of one
  ([parsing.md](./parsing.md#nothing-falls-back-so-a-failed-parse-must-surface)).
  Without it, a line says PDFs stay unparsed.
- **Subjects**: with Canvas connected (`get_auth_status`), the step shows the
  stored subjects and calls `sync_subjects` once. The Sync page usually
  upserts its `subjects-loaded` answer, but it isn't mounted during setup, so
  `useCanvasSubjects` does; new subjects arrive selected only if current-term,
  and a re-run keeps earlier choices. This term's subjects show as
  `SubjectRow`s, past ones behind a toggle; a toggle writes
  `setSubjectSelected` at once. Without Canvas, one line says nothing syncs
  until it is connected.
- **Leaving starts the sync**, by Next or Skip alike, when Canvas is
  connected and a subject is selected (Next then reads *Start syncing*);
  both keys are optional, so Next is always enabled. Keys come first so PDFs
  don't fail for a missing token on the first run. `startFirstSync`
  (`library/firstSync.ts`) leaves a sync already running alone and keeps any
  other refusal for `SyncProgressLine`, which the Agent step shows: the run's
  subject, phase and count from `useSyncStore`, then whether it finished or
  stopped.

## The Agent step lists every agent as an equal

Four `OptionCard`s, one per agent, each with its `ProviderMark`, a cost line,
a quieter plan note and a link to the vendor's plans (or OpenRouter's keys
page). None is badged as the pick: the order is fixed (Claude Code, Codex,
Antigravity, opencode) except that ready agents come first, sorted once when
`useBridgeHealth` and `useSignInStatus` first both answer, so a card never
moves after a sign-in. The prices are the vendors' own pages on 2026-10-12,
in USD (none lists AUD); a number a vendor doesn't publish is left out
(`agentOptions.ts`).

- **Claude Code, Codex, Antigravity** show *Checking…*, *Not installed* with
  *Install* (`InstallAgentDialog`), *Signed out* with *Sign in*
  (`SignInDialog`), or *Ready* with the account, the card selected. A CLI
  whose sign-in check couldn't answer shows *Installed*.
- **opencode** shows *Not installed* with *Install*, or *Ready*, the card
  selected: it is ready once installed, the gate's rule, because its sign-in
  can't be asked without starting its server. It keeps a secondary
  *Connect OpenRouter* with a muted hint that it is only needed if opencode
  has no provider yet. The button opens `OpencodeConnectDialog` on a fixed
  OpenRouter entry (the plain API-key method opencode gives every provider
  that declares none), so nothing lists providers and opencode starts only on
  that click ([harness.md](./harness.md#opencode-providers-go-through-its-own-server)).
  A saved key shows *Connected* for the rest of setup.

The install and sign-in runs are held by the step, as Settings → Agents holds
them, so closing a dialog mid-run keeps it going. Every dialog's finish, and
*Check again*, rechecks both hooks. Next is enabled once any agent is
ready by `isAgentReady` (`app/src/lib/onboarding/gate.ts`), the same rule as
the gate, or OpenRouter was connected during this setup. Skip is always there.

## *Run setup again* reopens it in place

Settings → Canvas → Setup writes `{ completedAt: null, skipped: [] }` and
opens onboarding at once, without a reload. Quitting before Done leaves that
row, so the next launch opens it again.

## Gotchas

- Plain external links open through `AppLayout`'s capture handler, which isn't
  mounted during setup — a step that links out opens it itself.
- Reopening unmounts `AppLayout` and every tab with it; the tabs come back
  from the persisted strip when setup finishes.
