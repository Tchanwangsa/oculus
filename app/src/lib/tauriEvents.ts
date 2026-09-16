/**
 * Side-effect module: repairs Tauri's `unlisten` so a stale unsubscribe cannot
 * take down the app. Imported for its side effect only, and imported *first* in
 * `main.tsx` — ES imports evaluate in order, so it has to land before anything
 * that can call `listen()`.
 *
 * Tauri injects this into every document (`tauri/src/event/mod.rs`,
 * `unlisten_js_script`):
 *
 *     const listeners = (window[LISTENERS_OBJ] || {})[event]
 *     if (listeners) {
 *       window.__TAURI_INTERNALS__.unregisterCallback(listeners[eventId].handlerId)
 *     }
 *
 * The guard checks that the *event* has a listener map, then dereferences
 * `listeners[eventId]` without checking that this particular id is still in it.
 * The dispatch path one function down does guard the identical lookup
 * (`const listener = listeners[id]; if (listener)`) — the unlisten path just
 * never got the same treatment.
 *
 * The id goes missing because registration travels over a different channel
 * from the id that names it. `listen_js` evals the script that defines
 * `listeners[id]` into the webview and *separately* returns the id over IPC, so
 * an unlisten that arrives between the two finds the map (some other listener
 * on that event name put it there) and no entry of its own. React StrictMode
 * makes that window easy to hit: it mounts, runs the effect, and unmounts
 * again immediately, which is precisely a listen followed at once by unlisten.
 *
 * The throw happens *before* `invoke('plugin:event|unlisten')`, so catching it
 * at the call site — which six of our cleanups already do — silences the
 * rejection but leaks the subscription on the Rust side, and the backend goes
 * on dispatching into a dead handler. Swallowing it here instead lets
 * `_unlisten` continue to the IPC call and actually unregister.
 */
type EventInternals = {
  unregisterListener?: (event: string, eventId: number) => void;
};

const internals = (
  window as unknown as { __TAURI_EVENT_PLUGIN_INTERNALS__?: EventInternals }
).__TAURI_EVENT_PLUGIN_INTERNALS__;

// Absent outside the webview — the Vite dev server serves this app to a plain
// browser too, and there is nothing to repair there.
if (internals && typeof internals.unregisterListener === "function") {
  const original = internals.unregisterListener;
  internals.unregisterListener = (event, eventId) => {
    try {
      original(event, eventId);
    } catch {
      // Already gone, or never landed. Either way there is no callback left to
      // unregister, and the `plugin:event|unlisten` call after this one is the
      // half that still matters.
    }
  };
  if (internals.unregisterListener === original) {
    console.warn("[oculus] could not patch Tauri unregisterListener; unlisten may throw");
  }
}
