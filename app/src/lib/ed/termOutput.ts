/**
 * Output bound for an xterm view in the workspace's drawer: Ed's base64
 * frames as bytes, our own notes as dim lines, and a sink that queues what
 * arrives before the view mounts (xterm loads on demand).
 */

/** Ed's base64 frame as the bytes it carries, for `Terminal.write`. */
export function decodeBase64(data: string): Uint8Array {
  const bin = atob(data);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

/** What a sink writes to: the xterm view, once it mounts. */
export interface TermView {
  write(data: Uint8Array | string): void;
  /** Clears the screen and scrollback. */
  reset(): void;
  /** The fitted size; null until the view has laid out. */
  size(): { cols: number; rows: number } | null;
  focus(): void;
}

export type TermSink = ReturnType<typeof createTermSink>;

const LF = 0x0a;

export function createTermSink() {
  let view: TermView | null = null;
  const queue: Array<Uint8Array | string> = [];
  /** Whether the output so far ends a line, so a note starts on its own. */
  let atLineStart = true;

  function write(data: Uint8Array | string) {
    if (data.length === 0) return;
    atLineStart = typeof data === "string" ? data.endsWith("\n") : data[data.length - 1] === LF;
    if (view) view.write(data);
    else queue.push(data);
  }

  return {
    attach(next: TermView) {
      view = next;
      for (const data of queue.splice(0)) next.write(data);
    },
    detach(gone: TermView) {
      if (view === gone) view = null;
    },
    write,
    /** `text` dim on a line of its own, then a fresh line. */
    note(text: string) {
      const body = text.replace(/\r?\n/g, "\r\n");
      write(`${atLineStart ? "" : "\r\n"}\x1b[2m${body}\x1b[22m\r\n`);
    },
    reset() {
      queue.length = 0;
      atLineStart = true;
      view?.reset();
    },
    size: () => view?.size() ?? null,
    focus: () => view?.focus(),
  };
}

export interface RunExit {
  code: number | null;
  signal: string | null;
  message: string | null;
}

/** The dim lines a finished run ends with: Ed's message, then how it ended. */
export function runExitLines({ code, signal, message }: RunExit): string[] {
  const lines: string[] = [];
  if (message?.trim()) lines.push(message.trimEnd());
  if (code != null) lines.push(`Program exited with code ${code}`);
  else if (signal) lines.push(`Program stopped by signal ${signal}`);
  return lines;
}
