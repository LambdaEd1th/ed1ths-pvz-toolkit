return (async function () {
    window.pamNativeStageHost?.destroy();

    const canvas = document.getElementById("pam-stage-canvas");
    if (!canvas) {
        dioxus.send({ type: "error", message: "Native stage element is unavailable" });
        return;
    }

    const reportBounds = () => {
        const rect = canvas.getBoundingClientRect();
        const stage = canvas.closest(".pam-stage");
        const style = stage ? getComputedStyle(stage) : null;
        const radius = (property) => Math.max(0, Number.parseFloat(style?.[property] ?? "0") || 0);
        dioxus.send({
            type: "bounds",
            x: rect.left,
            y: rect.top,
            width: Math.max(1, rect.width),
            height: Math.max(1, rect.height),
            window_width: Math.max(1, window.innerWidth),
            window_height: Math.max(1, window.innerHeight),
            corner_radii: [
                radius("borderTopLeftRadius"),
                radius("borderTopRightRadius"),
                radius("borderBottomRightRadius"),
                radius("borderBottomLeftRadius"),
            ],
        });
    };

    const observer = new ResizeObserver(reportBounds);
    observer.observe(canvas);
    window.addEventListener("resize", reportBounds);

    const host = {
        destroy() {
            observer.disconnect();
            window.removeEventListener("resize", reportBounds);
            if (window.pamNativeStageHost === host) window.pamNativeStageHost = null;
        },
    };
    window.pamNativeStageHost = host;
    reportBounds();
    await new Promise(() => {});
})();
