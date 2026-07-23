const runtimeVersion = "20260723-render-worker-5";
let handle = null;
let framePending = false;
let runtimePromise = null;

function runtimeReady() {
    runtimePromise ??= (async () => {
        const runtime = await import(
            "./pkg/pam_viewer_renderer.js?v=20260723-render-worker-5"
        );
        await runtime.default({
            module_or_path: new URL(
                `./pkg/pam_viewer_renderer_bg.wasm?v=${runtimeVersion}`,
                import.meta.url,
            ),
        });
        return runtime;
    })();
    return runtimePromise;
}

async function supportsWebGpu() {
    if (
        typeof OffscreenCanvas === "undefined" ||
        typeof self.navigator?.gpu === "undefined"
    ) {
        return false;
    }
    let timeout = 0;
    try {
        const adapter = await Promise.race([
            self.navigator.gpu.requestAdapter(),
            new Promise((resolve) => {
                timeout = self.setTimeout(() => resolve(null), 2_000);
            }),
        ]);
        return Boolean(adapter);
    } catch (_) {
        return false;
    } finally {
        self.clearTimeout(timeout);
    }
}

function errorMessage(error) {
    return error instanceof Error ? error.message : String(error);
}

function scheduleFrame() {
    if (!handle || framePending) return;
    framePending = true;
    const callback = () => {
        framePending = false;
        if (!handle) return;
        try {
            handle.frame();
        } catch (error) {
            self.postMessage({ type: "error", message: errorMessage(error) });
        }
    };
    if (typeof self.requestAnimationFrame === "function") {
        self.requestAnimationFrame(callback);
    } else {
        self.setTimeout(callback, 0);
    }
}

self.onmessage = async (event) => {
    const message = event.data ?? {};
    try {
        switch (message.type) {
            case "probe": {
                const supported = await supportsWebGpu();
                self.postMessage({ type: "probe", supported });
                break;
            }
            case "init": {
                const runtime = await runtimeReady();
                handle = new runtime.RendererHandle();
                await handle.start_offscreen(
                    message.canvas,
                    message.width,
                    message.height,
                );
                self.postMessage({ type: "ready" });
                scheduleFrame();
                break;
            }
            case "scene":
                handle?.set_scene(message.scene);
                scheduleFrame();
                break;
            case "view":
                handle?.set_view(message.view);
                scheduleFrame();
                break;
            case "resize":
                handle?.resize(message.width, message.height);
                scheduleFrame();
                break;
            case "destroy":
                handle?.destroy();
                handle = null;
                self.close();
                break;
            default:
                break;
        }
    } catch (error) {
        self.postMessage({ type: "error", message: errorMessage(error) });
    }
};
