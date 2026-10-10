import { buildCli, stageCli } from "./runtime.mjs";

// Staged so the next build of the crate copies this CLI, not an older one, into target/.
// A release build here is a developer's (`cli:install` links it from anywhere), so it
// may look for libpdfium outside a bundle. `stage-cli`, which builds the bundle's CLI,
// does not pass the feature and rewrites the sidecar.
const debug = process.argv.includes("--debug");
stageCli(buildCli(debug ? "debug" : "release", debug ? [] : ["dev-pdfium"]));
