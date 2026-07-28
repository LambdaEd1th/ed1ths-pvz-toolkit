const ready = (async () => {
    const runtime = await import(
        "./pkg/rsb_preview_worker.js?v=20260728-rgba8-only-api-1"
    );
    await runtime.default({
        module_or_path: new URL(
            "./pkg/rsb_preview_worker_bg.wasm?v=20260728-rgba8-only-api-1",
            import.meta.url,
        ),
    });
    return runtime;
})();

function collectTransferables(value, transfer = [], seen = new Set()) {
    if (!value || typeof value !== "object") return transfer;
    if (ArrayBuffer.isView(value)) {
        if (!seen.has(value.buffer)) {
            seen.add(value.buffer);
            transfer.push(value.buffer);
        }
        return transfer;
    }
    if (value instanceof ArrayBuffer && !seen.has(value)) {
        seen.add(value);
        transfer.push(value);
        return transfer;
    }
    for (const child of Array.isArray(value) ? value : Object.values(value)) {
        collectTransferables(child, transfer, seen);
    }
    return transfer;
}

self.onmessage = async (event) => {
    const id = event.data?.id;
    try {
        const runtime = await ready;
        const request = event.data.request;
        const response = request.kind === "unpack"
            ? runtime.unpack_packet_preview(request.payload)
            : request.kind === "encode"
                ? runtime.encode_texture_preview(request.payload)
                : runtime.decode_preview(request.payload);
        self.postMessage(
            { id, ok: true, response },
            collectTransferables(response),
        );
    } catch (error) {
        self.postMessage({
            id,
            ok: false,
            error: error instanceof Error ? error.message : String(error),
        });
    }
};
