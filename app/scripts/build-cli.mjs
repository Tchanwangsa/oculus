import { buildCli } from "./runtime.mjs";

buildCli(process.argv.includes("--debug") ? "debug" : "release");
