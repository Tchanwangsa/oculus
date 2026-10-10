/**
 * Video transcription (`app/src-tauri/src/transcribe/`), the Groq key it
 * runs on, and the `transcribe` settings row: the engine order, the one
 * language, and each engine's switch. Rust writes `<video>.vtt` beside the
 * video and records nothing; a caller that tracks transcripts in the DB
 * stores the returned path itself.
 */

export * from "./engines";
export * from "./apple";
export * from "./whisper";
export * from "./settings";
export * from "./language";
