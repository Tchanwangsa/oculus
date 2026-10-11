import { useState, type ReactNode } from "react";
import { ArrowSquareOut, CaretDown, CaretRight } from "@phosphor-icons/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { AutoSignIn } from "@/components/settings/web/AutoSignIn";
import { Button } from "@/components/ui/button";

/** Okta's own settings page, where a factor is enrolled. */
const OKTA_SETTINGS = "https://sso.unimelb.edu.au/enduser/settings";

/**
 * The optional automated sign-in, folded away by default: what it does, who
 * can't use it, how to get the authenticator key by re-enrolling, then the
 * same `AutoSignIn` form as Settings → Canvas without its own title and guide.
 */
export function NeverSignInAgain() {
  const [open, setOpen] = useState(false);
  const Caret = open ? CaretDown : CaretRight;

  return (
    <section className="flex flex-col gap-3">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="flex w-fit cursor-pointer items-center gap-1.5 text-[13px] font-medium text-foreground transition-colors hover:text-brand"
      >
        <Caret size={12} className="shrink-0 text-muted-foreground" />
        Never sign in again
        <span className="font-normal text-muted-foreground">· optional</span>
      </button>

      {open && (
        <div className="flex flex-col gap-4 rounded-xl border border-border-subtle bg-surface/40 p-4">
          <div className="flex flex-col gap-1.5 text-xs/relaxed text-muted-foreground">
            <p>
              When your session runs out, Oculus signs in for you with your username, password
              and authenticator key, kept in the macOS keychain.
            </p>
            <p>
              It needs Google Authenticator as a factor. Accounts that only use Okta Verify push
              can't use it.
            </p>
          </div>

          <ol className="flex flex-col gap-2.5 text-xs/relaxed text-foreground">
            <GuideStep n={1}>
              <span className="flex flex-wrap items-center gap-2">
                Open your Okta settings.
                <Button variant="outline" size="xs" onClick={() => void openUrl(OKTA_SETTINGS)}>
                  Okta settings
                  <ArrowSquareOut />
                </Button>
              </span>
            </GuideStep>
            <GuideStep n={2}>Add Google Authenticator as a factor.</GuideStep>
            <GuideStep n={3}>
              On the QR code screen, click <q>Can't scan?</q> to show the key.
            </GuideStep>
            <GuideStep n={4}>
              Scan the QR code with your phone too, so both hold the key.
            </GuideStep>
            <GuideStep n={5}>
              Paste the key below — or the whole <code>otpauth://</code> link.
            </GuideStep>
          </ol>

          <AutoSignIn embedded />
        </div>
      )}
    </section>
  );
}

function GuideStep({ n, children }: { n: number; children: ReactNode }) {
  return (
    <li className="flex items-start gap-2.5">
      <span className="flex size-5 shrink-0 items-center justify-center rounded-full bg-brand-muted text-[11px] font-medium text-brand tabular-nums">
        {n}
      </span>
      <span className="min-h-5 pt-0.5">{children}</span>
    </li>
  );
}
