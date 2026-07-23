const runtimeVersion = "20260723-worker-runtime-3";
const ready = (async () => {
    const runtime = await import(
        "./pkg/pam_viewer_worker.js?v=20260723-worker-runtime-3"
    );
    await runtime.default({
        module_or_path: new URL(
            `./pkg/pam_viewer_worker_bg.wasm?v=${runtimeVersion}`,
            import.meta.url,
        ),
    });
    return runtime;
})();

function responseTransferList(response) {
    const transfer = [];
    const seen = new Set();
    const visit = (value) => {
        if (!value || typeof value !== "object") return;
        if (value instanceof Uint8Array) {
            if (!seen.has(value.buffer)) {
                seen.add(value.buffer);
                transfer.push(value.buffer);
            }
            return;
        }
        if (Array.isArray(value)) {
            for (const item of value) visit(item);
            return;
        }
        for (const nested of Object.values(value)) visit(nested);
    };
    visit(response);
    return transfer;
}

self.onmessage = async (event) => {
    const id = event.data?.id;
    try {
        const runtime = await ready;
        const response = await runtime.perform(event.data.request);
        self.postMessage(
            { id, ok: true, response },
            responseTransferList(response),
        );
    } catch (error) {
        self.postMessage({
            id,
            ok: false,
            error: error instanceof Error ? error.message : String(error),
        });
    }
};
