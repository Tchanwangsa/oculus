import { useEffect, useRef } from "react";

/** A rolling trace of actual microphone amplitude, with no React frame updates. */
export function VoiceWaveform({ analyser }: { analyser: AnalyserNode | null }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const el = canvas.current;
    if (!el || !analyser) return;
    const ctx = el.getContext("2d");
    if (!ctx) return;
    const samples = new Uint8Array(analyser.fftSize);
    const history: number[] = [];
    let frame = 0;
    let last = 0;
    const draw = (time: number) => {
      frame = requestAnimationFrame(draw);
      if (time - last < 45) return;
      last = time;
      analyser.getByteTimeDomainData(samples);
      const level = Math.sqrt(samples.reduce((sum, n) => sum + ((n - 128) / 128) ** 2, 0) / samples.length);
      const width = el.clientWidth;
      const height = el.clientHeight;
      const scale = window.devicePixelRatio || 1;
      el.width = width * scale; el.height = height * scale;
      ctx.scale(scale, scale);
      history.push(Math.min(1, level * 5));
      const count = Math.ceil(width / 5);
      if (history.length > count) history.splice(0, history.length - count);
      ctx.strokeStyle = getComputedStyle(el).color;
      ctx.lineWidth = 2; ctx.lineCap = "round";
      for (let i = 0; i < count; i++) {
        const h = Math.max(2, (history[history.length - count + i] ?? 0) * (height - 4));
        ctx.beginPath(); ctx.moveTo(i * 5 + 1, (height - h) / 2); ctx.lineTo(i * 5 + 1, (height + h) / 2); ctx.stroke();
      }
    };
    frame = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(frame);
  }, [analyser]);
  return <canvas ref={canvas} aria-hidden="true" className="h-7 min-w-0 flex-1 text-muted-foreground" />;
}
