import { useEffect, useRef } from "react";
import { listen, type EventCallback } from "@tauri-apps/api/event";

/** Subscribes to a Tauri event for the component's life. The handler is read
 *  through a ref, so a fresh closure each render neither resubscribes nor goes
 *  stale; an unmount before `listen` resolves still unlistens once it does. */
export function useTauriEvent<T = unknown>(name: string, handler: EventCallback<T>) {
  const ref = useRef(handler);
  ref.current = handler;
  useEffect(() => {
    const pending = listen<T>(name, (e) => ref.current(e));
    return () => void pending.then((off) => off()).catch(() => {});
  }, [name]);
}

/** `window` listener for the component's life, on one event name or several
 *  (the app's `*_UPDATED_EVENT`s). The handler is read through a ref. */
export function useWindowEvent(names: string | readonly string[], handler: (e: Event) => void) {
  const ref = useRef(handler);
  ref.current = handler;
  const key = typeof names === "string" ? names : names.join("\n");
  useEffect(() => {
    const list = key.split("\n");
    const fn = (e: Event) => ref.current(e);
    for (const n of list) window.addEventListener(n, fn);
    return () => {
      for (const n of list) window.removeEventListener(n, fn);
    };
  }, [key]);
}
