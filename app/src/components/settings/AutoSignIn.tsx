import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CircleNotch, Info, Key, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

type CredentialStatus = {
  username: string | null;
  has_password: boolean;
  has_totp: boolean;
};

/**
 * Stores the credentials that let Oculus answer the Okta sign-in itself.
 *
 * The setup key is the part people get stuck on: a TOTP code is a one-way
 * function of a seed, so it cannot be recovered from codes the app is already
 * showing. The only source is the enrolment screen, which is why the hint
 * points there rather than explaining the maths.
 */
export function AutoSignIn({ onSignedIn }: { onSignedIn?: () => void }) {
  const [status, setStatus] = useState<CredentialStatus | null>(null);
  const [editing, setEditing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [secret, setSecret] = useState("");

  const refresh = useCallback(async () => {
    try {
      setStatus(await invoke<CredentialStatus>("okta_credential_status"));
    } catch {
      /* leave the previous state */
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const configured = !!status?.has_password && !!status?.has_totp;

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await invoke("okta_save_credentials", {
        username,
        password,
        totpSecret: secret,
      });
      // Prove it works now rather than at 3am when a sync needs it.
      await invoke<string>("okta_sign_in");
      setPassword("");
      setSecret("");
      setEditing(false);
      await refresh();
      onSignedIn?.();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const forget = async () => {
    setBusy(true);
    try {
      await invoke("okta_clear_credentials");
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div>
      <div className="flex items-center justify-between gap-4 py-2">
        <Label className="text-xs font-normal text-foreground flex items-center gap-1.5">
          Sign in without the browser
          <Tooltip>
            <TooltipTrigger asChild>
              <Info size={12} className="text-muted-foreground/60" />
            </TooltipTrigger>
            <TooltipContent className="max-w-[260px]">
              Answers the university's password and authenticator prompts from
              the macOS keychain, so an expired session is rebuilt in the
              background instead of interrupting you. Needs Google
              Authenticator enrolled — push notifications cannot be automated.
            </TooltipContent>
          </Tooltip>
        </Label>
        <div className="flex items-center gap-1.5">
          {configured && !editing && (
            <span className="text-xs text-muted-foreground">
              {status?.username}
            </span>
          )}
          <Button
            variant={configured ? "ghost" : "default"}
            size="xs"
            onClick={() => setEditing((e) => !e)}
            disabled={busy}
          >
            <Key size={13} />
            {editing ? "Cancel" : configured ? "Replace" : "Set up"}
          </Button>
          {configured && !editing && (
            <Button
              variant="ghost"
              size="xs"
              className="text-destructive hover:text-destructive hover:bg-destructive/10"
              onClick={forget}
              disabled={busy}
            >
              <X size={13} /> Forget
            </Button>
          )}
        </div>
      </div>

      {editing && (
        <div className="flex flex-col gap-2 pb-2 pt-1">
          <Input
            className="h-7 text-xs"
            placeholder="Username"
            autoComplete="off"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
          />
          <Input
            className="h-7 text-xs"
            type="password"
            placeholder="Password"
            autoComplete="off"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
          <Input
            className="h-7 text-xs font-mono"
            type="password"
            placeholder="Authenticator setup key"
            autoComplete="off"
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
          />
          <p className="text-xs text-muted-foreground leading-relaxed">
            The setup key is shown only while enrolling an authenticator — open{" "}
            <span className="text-foreground">sso.unimelb.edu.au/enduser/settings</span>,
            set up Google Authenticator, and click “Can’t scan?” on the QR
            screen. Scan the QR with your phone too, so you keep a working
            authenticator.
          </p>
          <div className="flex items-center gap-1.5">
            <Button
              size="xs"
              onClick={save}
              disabled={busy || !username || !password || !secret}
            >
              {busy && <CircleNotch size={13} className="animate-spin" />}
              {busy ? "Verifying…" : "Save and test"}
            </Button>
          </div>
        </div>
      )}

      {error && <p className="text-xs text-destructive py-1">{error}</p>}
    </div>
  );
}
