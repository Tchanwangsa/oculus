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
| One file per step | `app/src/components/onboarding/WelcomeStep.tsx`, `app/src/components/onboarding/CanvasStep.tsx`, `app/src/components/onboarding/LibraryStep.tsx`, `app/src/components/onboarding/AgentStep.tsx`, `app/src/components/onboarding/DoneStep.tsx` |
| Settings → Canvas → Setup: *Run setup again* | `app/src/components/settings/web/SetupSection.tsx` |

## Onboarding replaces the shell, not a tab

`App` renders `Onboarding` instead of `AppLayout` while it is open, so the
persisted tab strip (`oculus-tabs`) is never touched and no tab route exists
for it. `EventBridge` stays mounted either way: backend events, the quality
sweep and the index queue run during setup. While the gate decides, `App`
paints a blank `bg-background` window, so neither onboarding nor a
"not connected" state flashes. `App` applies the stored page zoom at boot
(`app/src/lib/ui/pageZoom.ts`), so setup renders at the shell's size; the
zoom shortcuts live in `AppLayout` and don't work during setup.

## The settings row decides, and existing setups never see it

One `settings` row, `onboarding`, JSON `{ completedAt, skipped }`, where
`skipped` holds step ids. At launch, once per window load:

1. `completedAt` set → the shell.
2. Row present with `completedAt: null` → onboarding (Settings asked for it).
3. Row absent, or malformed → the existing-setup probe. Canvas authenticated
   (`get_auth_status`, not `useAuth`, which starts at "disconnected") and at
   least one agent ready → write `completedAt` silently and show the shell.
   Otherwise → onboarding.

An agent is ready when installed and signed in. opencode's sign-in can't be
asked from outside its own page, so installed is enough for it, and the gate
never starts opencode ([harness.md](./harness.md)). The probe asks Canvas
first and skips the agents when it is not signed in, because the sign-in
check spawns every CLI. It reads through `loadBridgeHealth` and
`loadSignInStatus`, filling the same caches `useBridgeHealth` and
`useSignInStatus` read, so the shell's pickers don't ask again. A gate that
fails shows the shell.

## Steps share one interface and one frame

The order is `welcome`, `canvas`, `library`, `agent`, `done`. Every step takes
`StepProps` (`types.ts`): `{ onNext, onSkip }`. A step renders only its body
inside `StepFrame`, which draws the count ("Step 2 of 3", the three steps
between Welcome and Done), the title, the description and the Back / Skip /
Next footer; the step passes `nextDisabled` to decide when Next is enabled.
Back and the count come from the shell through `stepNav.ts`, not props.

Skip adds the step to `skipped`; Next removes it. Next on Done writes
`completedAt` and the skipped list, and the shell returns. The Canvas,
Library and Agent steps render a title and a one-line description.

## *Run setup again* reopens it in place

Settings → Canvas → Setup writes `{ completedAt: null, skipped: [] }` and
opens onboarding at once, without a reload. Quitting before Done leaves that
row, so the next launch opens it again.

## Gotchas

- Plain external links open through `AppLayout`'s capture handler, which isn't
  mounted during setup — a step that links out opens it itself.
- Reopening unmounts `AppLayout` and every tab with it; the tabs come back
  from the persisted strip when setup finishes.
