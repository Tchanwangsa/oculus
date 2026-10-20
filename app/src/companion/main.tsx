import React from "react";
import ReactDOM from "react-dom/client";
import "./companion.css";
import { takeBrowserShareCredentials } from "@/lib/companion/link";
import { App } from "./App";

/**
 * The page a paired browser opens from a share link (docs/device-sharing.md).
 * A separate Vite entry (`companion.html`, `vite.companion.config.ts`): nothing
 * here may import Tauri, which `tests/repo/companionGraph.test.ts` checks.
 */

// First, before anything renders: take the secret out of the address bar.
const credentials = takeBrowserShareCredentials();

// The desktop theme lives in Tauri-backed prefs; a paired device follows its own system.
const dark = window.matchMedia?.("(prefers-color-scheme: dark)");
const applyScheme = () => document.documentElement.classList.toggle("dark", !!dark?.matches);
applyScheme();
dark?.addEventListener?.("change", applyScheme);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App credentials={credentials} />
  </React.StrictMode>,
);
