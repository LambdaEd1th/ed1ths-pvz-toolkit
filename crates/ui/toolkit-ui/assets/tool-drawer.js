(() => {
  const VERSION = 1;
  const SELECTOR = ".ui-tool-drawer-handle";
  const STORAGE_KEY = "ed1ths-pvz-toolkit.tool-drawer-handle-ratio.v1";
  const EDGE_INSET = 12;
  const DRAG_THRESHOLD = 4;
  const existing = window.toolkitToolDrawerHandleDrag;

  if (existing?.version === VERSION) {
    existing.refresh();
    return;
  }
  existing?.destroy?.();

  let active = null;
  let savedRatio = null;
  let applyFrame = 0;

  try {
    const stored = Number.parseFloat(window.localStorage.getItem(STORAGE_KEY));
    if (Number.isFinite(stored)) savedRatio = Math.min(1, Math.max(0, stored));
  } catch (_) {}

  const findHandle = (target) =>
    target instanceof Element ? target.closest(SELECTOR) : null;

  const boundsFor = (handle) => {
    const root = handle.closest(".ui-tool-page-toolbar");
    if (!root) return null;
    const rootRect = root.getBoundingClientRect();
    const handleRect = handle.getBoundingClientRect();
    if (rootRect.height <= 0 || handleRect.height <= 0) return null;
    const min = Math.min(EDGE_INSET, Math.max(0, rootRect.height - handleRect.height));
    const max = Math.max(min, rootRect.height - handleRect.height - EDGE_INSET);
    return { rootRect, handleRect, min, max };
  };

  const ratioFromTop = (top, bounds) =>
    bounds.max > bounds.min ? (top - bounds.min) / (bounds.max - bounds.min) : 0;

  const applyRatio = (ratio) => {
    const clampedRatio = Math.min(1, Math.max(0, ratio));
    document.querySelectorAll(SELECTOR).forEach((handle) => {
      const bounds = boundsFor(handle);
      if (!bounds) return;
      const top = bounds.min + (bounds.max - bounds.min) * clampedRatio;
      handle.style.setProperty("--ui-tool-drawer-handle-top", `${top}px`);
    });
  };

  const scheduleApply = () => {
    if (savedRatio === null || applyFrame) return;
    applyFrame = window.requestAnimationFrame(() => {
      applyFrame = 0;
      applyRatio(savedRatio);
    });
  };

  const saveRatio = (ratio) => {
    savedRatio = Math.min(1, Math.max(0, ratio));
    applyRatio(savedRatio);
    try {
      window.localStorage.setItem(STORAGE_KEY, String(savedRatio));
    } catch (_) {}
  };

  const onPointerDown = (event) => {
    const handle = findHandle(event.target);
    if (!handle || event.isPrimary === false || event.button !== 0) return;
    const bounds = boundsFor(handle);
    if (!bounds) return;
    active = {
      handle,
      pointerId: event.pointerId,
      startY: event.clientY,
      startTop: bounds.handleRect.top - bounds.rootRect.top,
      dragged: false,
    };
    try {
      handle.setPointerCapture(event.pointerId);
    } catch (_) {}
  };

  const onPointerMove = (event) => {
    if (!active || active.pointerId !== event.pointerId) return;
    const delta = event.clientY - active.startY;
    if (!active.dragged && Math.abs(delta) < DRAG_THRESHOLD) return;
    active.dragged = true;
    active.handle.classList.add("is-dragging");
    const bounds = boundsFor(active.handle);
    if (!bounds) return;
    const top = Math.min(bounds.max, Math.max(bounds.min, active.startTop + delta));
    savedRatio = Math.min(1, Math.max(0, ratioFromTop(top, bounds)));
    applyRatio(savedRatio);
    event.preventDefault();
  };

  const finishPointer = (event) => {
    if (!active || active.pointerId !== event.pointerId) return;
    const { handle, pointerId, dragged } = active;
    active = null;
    handle.classList.remove("is-dragging");
    try {
      if (handle.hasPointerCapture(pointerId)) handle.releasePointerCapture(pointerId);
    } catch (_) {}
    if (dragged && savedRatio !== null) {
      try {
        window.localStorage.setItem(STORAGE_KEY, String(savedRatio));
      } catch (_) {}
    }
  };

  const moveFromKeyboard = (event) => {
    const handle = findHandle(event.target);
    if (!handle || !["ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) return;
    const bounds = boundsFor(handle);
    if (!bounds) return;
    const currentTop = bounds.handleRect.top - bounds.rootRect.top;
    const step = event.shiftKey ? 40 : 16;
    const top =
      event.key === "Home"
        ? bounds.min
        : event.key === "End"
          ? bounds.max
          : Math.min(
              bounds.max,
              Math.max(bounds.min, currentTop + (event.key === "ArrowUp" ? -step : step)),
            );
    saveRatio(ratioFromTop(top, bounds));
    event.preventDefault();
    event.stopPropagation();
  };

  const resolveInitialRatio = () => {
    if (savedRatio === null) {
      const handle = Array.from(document.querySelectorAll(SELECTOR)).find(
        (candidate) => boundsFor(candidate) !== null,
      );
      const bounds = handle ? boundsFor(handle) : null;
      if (handle && bounds) {
        const currentTop = bounds.handleRect.top - bounds.rootRect.top;
        const top = Math.min(bounds.max, Math.max(bounds.min, currentTop));
        savedRatio = Math.min(1, Math.max(0, ratioFromTop(top, bounds)));
      }
    }
  };
  const resizeObserver = new ResizeObserver(() => {
    resolveInitialRatio();
    scheduleApply();
  });
  const refresh = () => {
    document
      .querySelectorAll(".ui-tool-page-toolbar")
      .forEach((root) => resizeObserver.observe(root));
    resolveInitialRatio();
    scheduleApply();
  };
  const destroy = () => {
    if (applyFrame) window.cancelAnimationFrame(applyFrame);
    resizeObserver.disconnect();
    document.removeEventListener("pointerdown", onPointerDown, true);
    document.removeEventListener("pointermove", onPointerMove, true);
    document.removeEventListener("pointerup", finishPointer, true);
    document.removeEventListener("pointercancel", finishPointer, true);
    document.removeEventListener("lostpointercapture", finishPointer, true);
    document.removeEventListener("keydown", moveFromKeyboard, true);
  };

  document.addEventListener("pointerdown", onPointerDown, true);
  document.addEventListener("pointermove", onPointerMove, { capture: true, passive: false });
  document.addEventListener("pointerup", finishPointer, true);
  document.addEventListener("pointercancel", finishPointer, true);
  document.addEventListener("lostpointercapture", finishPointer, true);
  document.addEventListener("keydown", moveFromKeyboard, true);

  window.toolkitToolDrawerHandleDrag = { version: VERSION, refresh, destroy };
  refresh();
})();
