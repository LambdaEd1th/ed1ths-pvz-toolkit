// Shared focus containment for Toolkit overlays. Escape handling stays in
// Dioxus so each layer can decide whether it closes or defers to a menu.
(() => {
  const VERSION = 1;
  const previous = window.toolkitModalFocus;
  if (previous?.version === VERSION) {
    previous.refresh?.();
    return;
  }
  previous?.destroy?.();

  const FOCUSABLE = [
    "a[href]",
    "button:not([disabled])",
    "input:not([disabled]):not([type='hidden'])",
    "select:not([disabled])",
    "textarea:not([disabled])",
    "iframe",
    "object",
    "embed",
    "[contenteditable='true']",
    "[tabindex]:not([tabindex='-1'])",
  ].join(",");

  /** @type {{id: string, layer: Element, content: Element, previous: Element | null, inerted: Element[]}[]} */
  const entries = [];
  let lastPointerTarget = null;

  const isVisible = element => {
    if (!element?.isConnected) return false;
    if (element.closest("[inert], [hidden], [aria-hidden='true']")) return false;
    const style = getComputedStyle(element);
    if (style.display === "none" || style.visibility !== "visible") return false;
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  };

  const focusableElements = root =>
    Array.from(root.querySelectorAll(FOCUSABLE)).filter(isVisible);

  const inertSiblings = (node, excludeSelector) => {
    const inerted = [];
    for (let current = node; current && current !== document.body; current = current.parentElement) {
      const parent = current.parentElement;
      if (!parent) break;
      for (const sibling of parent.children) {
        if (sibling === current) continue;
        if (sibling.matches("script, style, link, meta, template")) continue;
        if (excludeSelector && sibling.matches(excludeSelector)) continue;
        if (sibling.inert) continue;
        sibling.inert = true;
        inerted.push(sibling);
      }
    }
    return inerted;
  };

  const topEntry = () => entries[entries.length - 1] || null;

  const focusEntry = entry => {
    const focusable = focusableElements(entry.content);
    if (focusable.length) {
      focusable[0].focus({ preventScroll: true });
      return;
    }
    if (!entry.content.hasAttribute("tabindex")) {
      entry.content.setAttribute("tabindex", "-1");
    }
    entry.content.focus({ preventScroll: true });
  };

  const onKeyDown = event => {
    if (event.key !== "Tab") return;
    const entry = topEntry();
    if (!entry) return;
    const focusable = focusableElements(entry.content);
    if (!focusable.length) {
      event.preventDefault();
      entry.content.focus({ preventScroll: true });
      return;
    }
    // Drive Tab ourselves so WebKit's platform tabbing preferences (which may
    // skip buttons entirely) cannot move focus out of the dialog.
    const active = document.activeElement;
    let index = focusable.indexOf(active);
    if (index < 0) index = event.shiftKey ? 0 : -1;
    const step = event.shiftKey ? -1 : 1;
    const next = focusable[(index + step + focusable.length) % focusable.length];
    event.preventDefault();
    next.focus({ preventScroll: true });
  };

  const deactivate = id => {
    const index = entries.findIndex(entry => entry.id === id);
    if (index < 0) return false;
    const wasTop = index === entries.length - 1;
    const [entry] = entries.splice(index, 1);
    for (const element of entry.inerted) element.inert = false;
    if (wasTop) {
      const previous = entry.previous;
      const target =
        previous?.isConnected && previous !== document.body
          ? previous
          : entry.layer.querySelector("[data-modal-restore-focus]");
      target?.focus?.({ preventScroll: true });
    }
    return true;
  };

  const rememberPointerTarget = event => {
    if (event.button !== 0) return;
    const target =
      event.target instanceof Element
        ? event.target.closest(
            "button, a[href], input, select, textarea, [role='button'], [tabindex]:not([tabindex='-1'])",
          )
        : null;
    if (target?.isConnected) lastPointerTarget = target;
  };

  const activate = options => {
    if (!options?.id) return false;
    deactivate(options.id);
    const layer = document.querySelector(options.selector);
    if (!layer) return false;

    if (options.modalSelector) {
      const marker = layer.querySelector(options.modalSelector);
      if (marker) {
        const style = getComputedStyle(marker);
        if (
          style.display === "none" ||
          style.visibility === "hidden" ||
          style.pointerEvents === "none"
        ) {
          return false;
        }
      }
    }

    const content = layer.querySelector("[data-modal-content]") || layer;
    const focused = document.activeElement;
    const entry = {
      id: options.id,
      layer,
      content,
      previous:
        focused && focused !== document.body && focused.isConnected
          ? focused
          : lastPointerTarget,
      inerted: inertSiblings(layer, options.excludeSelector),
    };
    entries.push(entry);
    focusEntry(entry);
    return true;
  };

  // WebKit does not focus buttons on click by default, which makes it
  // impossible to remember the trigger for focus restoration. Match the
  // behavior of other engines for primary-button clicks.
  const onPointerDown = event => {
    if (event.button !== 0) return;
    const target =
      event.target instanceof Element
        ? event.target.closest("button, [role='button']")
        : null;
    if (!target || target.disabled || target.tabIndex < 0) return;
    if (document.activeElement !== target) {
      target.focus({ preventScroll: true });
    }
  };

  const destroy = () => {
    document.removeEventListener("keydown", onKeyDown, true);
    document.removeEventListener("pointerdown", onPointerDown, true);
    document.removeEventListener("pointerdown", rememberPointerTarget, true);
    document.removeEventListener("mousedown", rememberPointerTarget, true);
    while (entries.length) {
      const entry = entries.pop();
      for (const element of entry.inerted) element.inert = false;
    }
    if (window.toolkitModalFocus === api) delete window.toolkitModalFocus;
  };

  document.addEventListener("keydown", onKeyDown, true);
  document.addEventListener("pointerdown", onPointerDown, true);
  document.addEventListener("pointerdown", rememberPointerTarget, true);
  document.addEventListener("mousedown", rememberPointerTarget, true);

  const api = {
    version: VERSION,
    activate,
    deactivate,
    destroy,
    refresh() {},
    activeIds() {
      return entries.map(entry => entry.id);
    },
  };
  window.toolkitModalFocus = api;
})();
