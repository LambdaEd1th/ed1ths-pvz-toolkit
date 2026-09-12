// Own tooltip lifetime instead of leaving native title bubbles in a WebView.
// Shared by every tool, including dynamically mounted/virtualized controls.
(() => {
  const VERSION = 1;
  const previous = window.toolkitTooltips;
  if (previous?.version === VERSION) {
    previous.refresh();
    return;
  }
  previous?.destroy?.();

  const ATTRIBUTE = "data-ui-tooltip";
  const SELECTOR = `[${ATTRIBUTE}]`;
  const DELAY = 450;
  const records = new WeakMap();
  const listeners = [];
  let owner = null;
  let mode = null;
  let blockedOwner = null;
  let pointer = null;
  let keyboard = false;
  let timer = 0;
  let guard = 0;
  let bubble = null;
  let descriptionOwner = null;
  let destroyed = false;

  const elementAt = (target) => target instanceof Element ? target : target?.parentElement;
  const findOwner = (target) => {
    const element = elementAt(target)?.closest(SELECTOR);
    return element?.getAttribute(ATTRIBUTE)?.trim() ? element : null;
  };
  const textFor = (element) => element?.getAttribute(ATTRIBUTE)?.trim() || "";

  const removeDescription = () => {
    if (!descriptionOwner || !bubble) return;
    const ids = (descriptionOwner.getAttribute("aria-describedby") || "").split(/\s+/)
      .filter((id) => id && id !== bubble.id);
    if (ids.length) descriptionOwner.setAttribute("aria-describedby", ids.join(" "));
    else descriptionOwner.removeAttribute("aria-describedby");
    descriptionOwner = null;
  };

  const hide = (block = false) => {
    if (block && owner) blockedOwner = owner;
    clearTimeout(timer);
    clearInterval(guard);
    timer = guard = 0;
    owner = mode = null;
    removeDescription();
    if (bubble) {
      try { bubble.hidePopover?.(); } catch (_) {}
      bubble.hidden = true;
      bubble.textContent = "";
    }
  };

  const isVisible = (element) => {
    if (!element?.isConnected || document.hidden) return false;
    if (element.closest('[hidden], [inert], [aria-hidden="true"]')) return false;
    const rect = element.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0 || rect.bottom <= 0 || rect.right <= 0 ||
        rect.top >= window.innerHeight || rect.left >= window.innerWidth) return false;
    for (let node = element; node; node = node.parentElement) {
      const style = getComputedStyle(node);
      if (style.display === "none" || style.visibility !== "visible" || Number(style.opacity) === 0) return false;
    }
    return true;
  };

  const valid = () => {
    if (!owner || !textFor(owner) || !isVisible(owner)) return false;
    if (mode === "focus") {
      const rect = owner.getBoundingClientRect();
      const hit = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
      return owner.contains(document.activeElement) && owner.contains(hit);
    }
    if (!pointer) return false;
    // Hit testing also catches overlays and CSS transitions without pointerout.
    return owner.contains(document.elementFromPoint(pointer.x, pointer.y));
  };

  const ensureBubble = () => {
    if (bubble?.isConnected) return bubble;
    bubble = document.createElement("div");
    bubble.id = "toolkit-shared-tooltip";
    bubble.className = "ui-tooltip";
    bubble.setAttribute("role", "tooltip");
    bubble.hidden = true;
    // A popover sits above modal top layers; fixed positioning is the fallback.
    if (typeof bubble.showPopover === "function") bubble.setAttribute("popover", "manual");
    document.body.append(bubble);
    return bubble;
  };

  const show = () => {
    timer = 0;
    if (!valid()) { hide(); return; }
    const tip = ensureBubble();
    tip.textContent = textFor(owner); // Never interpret a file name/title as markup.
    tip.hidden = false;
    try { tip.showPopover?.(); } catch (_) {}
    const rect = owner.getBoundingClientRect();
    const tipRect = tip.getBoundingClientRect();
    const left = Math.max(8, Math.min(window.innerWidth - tipRect.width - 8,
      rect.left + (rect.width - tipRect.width) / 2));
    const below = rect.bottom + 8;
    const top = below + tipRect.height <= window.innerHeight - 8
      ? below : Math.max(8, rect.top - tipRect.height - 8);
    tip.style.left = `${left}px`;
    tip.style.top = `${top}px`;
    descriptionOwner = owner;
    const ids = new Set((owner.getAttribute("aria-describedby") || "").split(/\s+/).filter(Boolean));
    ids.add(tip.id);
    owner.setAttribute("aria-describedby", [...ids].join(" "));
  };

  const schedule = (element, nextMode) => {
    if (blockedOwner && blockedOwner !== element) blockedOwner = null;
    if (!element || blockedOwner === element) { hide(); return; }
    if (owner === element && mode === nextMode) return;
    hide();
    owner = element;
    mode = nextMode;
    if (!valid()) { hide(); return; }
    timer = window.setTimeout(show, DELAY);
    // A bounded, active-only check catches removal/animation/occlusion even when
    // the browser never dispatches a leave event (e.g. native file dialogs).
    guard = window.setInterval(() => { if (!valid()) hide(true); }, 100);
  };

  const normalizeTitle = (element, explicitChange = false) => {
    const value = element.getAttribute("title");
    const old = records.get(element);
    if (!value?.trim()) {
      if (explicitChange && old) {
        element.removeAttribute(ATTRIBUTE);
        if (old.label && element.getAttribute("aria-label") === old.label) element.removeAttribute("aria-label");
        records.delete(element);
        if (owner === element) hide(true);
      }
      return;
    }
    let label = old?.label || null;
    if (label && element.getAttribute("aria-label") !== label) label = null;
    const needsName = element.matches('button, a, input, select, textarea, [role="button"]') &&
      !element.hasAttribute("aria-labelledby") && !element.textContent.trim();
    if (label || (needsName && !element.hasAttribute("aria-label"))) {
      label = value;
      element.setAttribute("aria-label", value);
    }
    records.set(element, { value, label });
    element.setAttribute(ATTRIBUTE, value);
    // Empty (not absent) title suppresses inherited native titles as well.
    element.setAttribute("title", "");
    if (owner === element) hide(true);
  };

  const scan = (root) => {
    if (root instanceof Element && root.hasAttribute("title")) normalizeTitle(root);
    root.querySelectorAll?.("[title]").forEach((element) => normalizeTitle(element));
  };
  const observer = new MutationObserver((mutations) => {
    // Do not feed our attribute normalization back into Dioxus's updates. The
    // callback is synchronous, so application changes cannot interleave here.
    observer.disconnect();
    try {
      const changedTitles = new Set();
      const added = [];
      for (const mutation of mutations) {
        if (mutation.type === "attributes" && mutation.attributeName === "title") changedTitles.add(mutation.target);
        if (mutation.type === "childList") added.push(...mutation.addedNodes);
      }
      changedTitles.forEach((element) => normalizeTitle(element, true));
      added.forEach(scan);
      if (owner && (!valid() || (bubble && !bubble.hidden && bubble.textContent !== textFor(owner)))) hide(true);
    } finally { observe(); }
  });
  const observe = () => {
    if (!destroyed) observer.observe(document.documentElement, {
      subtree: true, childList: true, attributes: true,
      attributeFilter: ["title", ATTRIBUTE, "class", "style", "hidden", "inert", "aria-hidden", "open", "aria-expanded"],
    });
  };
  const refresh = () => {
    observer.disconnect();
    scan(document);
    observe();
  };

  const onPointer = (event) => {
    if (event.pointerType === "touch" || event.buttons) { hide(true); return; }
    pointer = { x: event.clientX, y: event.clientY };
    schedule(findOwner(event.target), "pointer");
  };
  const onOut = (event) => {
    if (!event.relatedTarget) { pointer = null; blockedOwner = null; hide(); }
    else if (owner && !owner.contains(elementAt(event.relatedTarget))) hide();
    if (blockedOwner && !blockedOwner.contains(elementAt(event.relatedTarget))) blockedOwner = null;
  };
  const dismiss = () => hide(true);
  const listen = (target, type, handler, options = true) => {
    target.addEventListener(type, handler, options);
    listeners.push(() => target.removeEventListener(type, handler, options));
  };
  listen(document, "pointerover", onPointer);
  listen(document, "pointermove", onPointer);
  listen(document, "pointerout", onOut);
  listen(document, "pointerdown", () => { keyboard = false; hide(true); });
  for (const type of ["pointercancel", "lostpointercapture", "click", "contextmenu", "dragstart", "scroll", "wheel"]) listen(document, type, dismiss);
  listen(document, "keydown", (event) => {
    keyboard = true;
    if (!["Shift", "Control", "Alt", "Meta"].includes(event.key)) hide(true);
  });
  listen(document, "focusin", (event) => {
    if (keyboard) schedule(findOwner(event.target), "focus");
  });
  listen(document, "focusout", dismiss);
  for (const type of ["blur", "resize", "pagehide"]) listen(window, type, dismiss);
  listen(document, "visibilitychange", dismiss);
  if (window.visualViewport) {
    listen(window.visualViewport, "resize", dismiss);
    listen(window.visualViewport, "scroll", dismiss);
  }
  window.toolkitTooltips = {
    version: VERSION, refresh, dismiss,
    destroy() {
      destroyed = true;
      hide();
      observer.disconnect();
      listeners.forEach((remove) => remove());
      document.querySelectorAll(SELECTOR).forEach((element) => {
        const record = records.get(element);
        if (!record) return;
        if (element.getAttribute("title") === "") element.setAttribute("title", record.value);
        if (record.label && element.getAttribute("aria-label") === record.label) element.removeAttribute("aria-label");
        element.removeAttribute(ATTRIBUTE);
      });
      bubble?.remove();
      if (window.toolkitTooltips?.version === VERSION) delete window.toolkitTooltips;
    },
  };
  refresh();
})();
