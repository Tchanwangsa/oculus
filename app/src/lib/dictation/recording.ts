/** One microphone owner across split panes. Tracks always stop on cancellation. */
let owner: symbol | null = null;

export interface Recording {
  analyser: AnalyserNode;
  finish: () => Promise<Blob>;
  cancel: () => void;
}

export async function recordVoice(): Promise<Recording> {
  if (owner) throw new Error("Another composer is using the microphone.");
  if (!navigator.mediaDevices?.getUserMedia || typeof MediaRecorder === "undefined") {
    throw new Error("Microphone recording is unavailable in this app. Update Oculus and try again.");
  }
  const token = Symbol();
  owner = token;
  let stream: MediaStream | null = null;
  let context: AudioContext | null = null;
  let recorder: MediaRecorder | null = null;
  let released = false;
  const release = () => {
    if (released) return;
    released = true;
    stream?.getTracks().forEach((track) => track.stop());
    void context?.close().catch(() => {});
    if (owner === token) owner = null;
  };
  try {
    stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true }, video: false });
    context = new AudioContext();
    const analyser = context.createAnalyser();
    analyser.fftSize = 256;
    context.createMediaStreamSource(stream).connect(analyser);
    const mimeType = ["audio/webm;codecs=opus", "audio/mp4", "audio/webm"].find((type) => MediaRecorder.isTypeSupported(type));
    recorder = new MediaRecorder(stream, mimeType ? { mimeType, audioBitsPerSecond: 64_000 } : undefined);
    const chunks: Blob[] = [];
    let bytes = 0;
    let failure: Error | null = null;
    recorder.ondataavailable = ({ data }) => {
      bytes += data.size;
      if (bytes > 24 * 1024 * 1024) {
        failure = new Error("This recording is too large. Dictate a shorter message.");
        if (recorder?.state !== "inactive") recorder?.stop();
      } else if (!failure) chunks.push(data);
    };
    const stopped = new Promise<Blob>((resolve, reject) => {
      recorder!.onerror = () => { failure = new Error("The microphone recording was interrupted. Try again."); };
      recorder!.onstop = () => {
        release();
        if (failure) reject(failure);
        else resolve(new Blob(chunks, { type: recorder!.mimeType }));
      };
    });
    // A device can stop recording before the user clicks Finish.
    void stopped.catch(() => {});
    recorder.start(1000);
    return {
      analyser,
      finish: () => {
        if (recorder!.state !== "inactive") recorder!.stop();
        return stopped;
      },
      cancel: () => {
        if (recorder!.state !== "inactive") recorder!.stop();
        release();
      },
    };
  } catch (cause) {
    release();
    if (cause instanceof DOMException && (cause.name === "NotAllowedError" || cause.name === "SecurityError")) {
      throw new Error("Microphone access is blocked. Allow Oculus to use your microphone in system privacy settings, then try again.");
    }
    if (cause instanceof DOMException && cause.name === "NotFoundError") {
      throw new Error("No microphone was found. Connect one and try again.");
    }
    throw cause;
  }
}
