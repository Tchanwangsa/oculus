import { buildCli, stageCli } from "./runtime.mjs";

// Staged so the next build of the crate copies this CLI, not an older one, into target/.
stageCli(buildCli(process.argv.includes("--debug") ? "debug" : "release"));
