return (async function () {
    window.pamResponsiveLayoutHost?.destroy();

    const root = document.querySelector(".pam-app");
    if (!root) {
        dioxus.send(false);
        return;
    }

    const report = () => dioxus.send(root.getBoundingClientRect().width <= 900);
    const observer = new ResizeObserver(report);
    const host = {
        destroy() {
            observer.disconnect();
            if (window.pamResponsiveLayoutHost === host) window.pamResponsiveLayoutHost = null;
        },
    };

    window.pamResponsiveLayoutHost = host;
    observer.observe(root);
    report();
    await new Promise(() => {});
})();
