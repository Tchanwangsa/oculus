/**
 * Side-effect module, imported first in `main.tsx`: makes Tauri's injected
 * `unregisterListener` tolerate an event id that is not in its listener map.
 * The id is returned over IPC separately from the script that registers it, so
 * an unlisten in between (StrictMode's mount/unmount) dereferences undefined.
 * That throw lands *before* `plugin:event|unlisten`, so catching it at the call
 * site leaks the Rust-side subscription; swallowing it here lets the IPC run.
 */
type EventInternals = {
  unregisterListener?: (event: string, eventId: number) => void;
};

const internals = (
  window as unknown as { __TAURI_EVENT_PLUGIN_INTERNALS__?: EventInternals }
).__TAURI_EVENT_PLUGIN_INTERNALS__;

// Absent in a plain browser on the Vite dev server.
if (internals && typeof internals.unregisterListener === "function") {
  const original = internals.unregisterListener;
  internals.unregisterListener = (event, eventId) => {
    try {
      original(event, eventId);
    } catch {
      // No callback left to unregister; the IPC unlisten after this still runs.
    }
  };
  if (internals.unregisterListener === original) {
    console.warn("[oculus] could not patch Tauri unregisterListener; unlisten may throw");
  }
}
