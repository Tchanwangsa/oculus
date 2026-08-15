"""One lock guarding every torch model load in this process.

Docling's layout/formula models and the Qwen embedder are loaded by different
threads — the quality-parse worker and the `/embed-pdf` handler — and parsing a
freshly scraped PDF kicks off both at the same moment. Loading them
concurrently fails:

    Failed to load model from .../docling-layout-heron:
    Cannot copy out of meta tensor; no data!

`from_pretrained` initialises on the meta device and then materialises weights
onto the real device. That two-step is not thread-safe: the losing thread finds
tensors already moved out from under it and gets a meta tensor with no storage.

Held only for construction, not inference — once loaded, both run concurrently.
"""

import threading

MODEL_INIT_LOCK = threading.Lock()
