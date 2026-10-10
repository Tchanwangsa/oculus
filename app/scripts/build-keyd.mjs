// `bun run keyd`: build and sign the dev oculus-keyd as its helper app, then —
// from the main checkout — install it when its source changed. The release
// build the bundle ships is stage-keyd.mjs; both share keyd-build.mjs.
import { buildKeyd, installFromMainCheckout } from "./keyd-build.mjs";

const built = buildKeyd("dev");
if (built) installFromMainCheckout(built.helper);
