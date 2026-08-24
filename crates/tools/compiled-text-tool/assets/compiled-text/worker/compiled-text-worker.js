const runtime = import("./pkg/compiled_text_worker.js?v=20260824-compiled-text-1").then(
  async (module) => {
    await module.default();
    return module;
  },
);

function collectTransferables(value, output = []) {
  if (value instanceof Uint8Array) {
    output.push(value.buffer);
    return output;
  }
  if (Array.isArray(value)) {
    for (const item of value) collectTransferables(item, output);
    return output;
  }
  if (value && typeof value === "object") {
    for (const item of Object.values(value)) collectTransferables(item, output);
  }
  return output;
}

self.onmessage = async ({ data: envelope }) => {
  const { id, request } = envelope;
  try {
    const module = await runtime;
    let response;
    if (request.kind === "decode") {
      response = module.decode_compiled_text(request.payload);
    } else if (request.kind === "encode") {
      response = module.encode_compiled_text(request.payload);
    } else {
      throw new Error(`Unknown Compiled Text worker request: ${request.kind}`);
    }
    self.postMessage({ id, ok: true, response }, collectTransferables(response));
  } catch (error) {
    self.postMessage({
      id,
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
  }
};
