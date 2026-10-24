import { useEffect, useRef, useState } from "react";

/** Reads again after committed writes; only the latest request may publish. */
export function useLiveData<T>(
  key: string,
  load: () => Promise<T>,
  events: string[] = [],
) {
  const [result, setResult] = useState<{
    key: string;
    value?: T;
    error?: string;
  }>({ key });
  const reader = useRef(load);
  reader.current = load;
  const eventKey = events.join("|");
  useEffect(() => {
    let request = 0;
    const refresh = () => {
      const current = ++request;
      reader
        .current()
        .then((value) => {
          if (current === request) setResult({ key, value });
        })
        .catch((error) => {
          if (current === request) setResult({ key, error: String(error) });
        });
    };
    refresh();
    const names = eventKey ? eventKey.split("|") : [];
    names.forEach((name) => window.addEventListener(name, refresh));
    return () => {
      request++;
      names.forEach((name) => window.removeEventListener(name, refresh));
    };
  }, [key, eventKey]);
  return result.key === key ? result : { key };
}
