const ready = (async () => {
  const runtime = await import("./pkg/rton_editor_worker.js");
  await runtime.default();
  return runtime;
})();

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function collectTransferables(value, transfer = [], seen = new Set()) {
  if (value === null || typeof value !== "object") return transfer;
  if (ArrayBuffer.isView(value)) {
    const buffer = value.buffer;
    if (buffer instanceof ArrayBuffer && !seen.has(buffer)) {
      seen.add(buffer);
      transfer.push(buffer);
    }
    return transfer;
  }
  if (value instanceof ArrayBuffer && !seen.has(value)) {
    seen.add(value);
    transfer.push(value);
  }
  return transfer;
}

async function dispatch(message) {
  const runtime = await ready;

  if (message.kind === "parse") {
    return runtime.rton_worker_parse(message.request);
  }
  if (message.kind === "rton-size") {
    return runtime.rton_worker_rton_size(message.request);
  }
  if (message.kind === "text-surface") {
    return runtime.rton_worker_text_surface(message.request);
  }
  if (message.kind === "text-surface-file") {
    const buffer = await message.file.arrayBuffer();
    return runtime.rton_worker_text_surface({
      bytes: new Uint8Array(buffer),
      format: message.format,
    });
  }
  if (message.kind === "open-text") {
    return runtime.rton_worker_open_text(message.request);
  }
  if (message.kind === "open-text-file") {
    const buffer = await message.file.arrayBuffer();
    return runtime.rton_worker_open_text({
      bytes: new Uint8Array(buffer),
      format: message.format,
      search_query: message.search_query ?? "",
    });
  }
  if (message.kind === "tree") {
    return runtime.rton_worker_tree(message.request);
  }
  if (message.kind === "value-search") {
    return runtime.rton_worker_value_search(message.request);
  }
  if (message.kind === "locate-text") {
    return runtime.rton_worker_locate_text(message.request);
  }
  if (message.kind === "text-search") {
    return runtime.rton_worker_text_search(message.request);
  }
  if (message.kind === "hex-search") {
    return runtime.rton_worker_hex_search(message.request);
  }
  if (message.kind === "release-document") {
    return runtime.rton_worker_release_document(message.request);
  }
  if (message.kind === "batch") {
    return runtime.rton_worker_batch(message.request);
  }
  return runtime.rton_worker_mode_switch(message.request);
}

self.onmessage = async (event) => {
  const id = event.data.id;
  try {
    const response = await dispatch(event.data);
    const transfer = [];
    collectTransferables(response?.surface?.RtonBytes, transfer);
    for (const item of response?.results ?? []) {
      collectTransferables(item.bytes, transfer);
    }
    self.postMessage({ id, ok: true, response }, transfer);
  } catch (error) {
    self.postMessage({ id, ok: false, error: errorMessage(error) });
  }
};
