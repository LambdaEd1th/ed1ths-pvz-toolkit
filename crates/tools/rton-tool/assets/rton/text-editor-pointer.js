(() => {
    const root = document.querySelector('.virtual-text-preview-content');
    const output = root?.querySelector('#rton-virtual-text-pointer');
    if (!root || !output || root.__rtonPointerInstalled) return;
    root.__rtonPointerInstalled = true;
    let dragging = false;

    const hitColumn = (line, x, y) => {
        let node, offset;
        if (document.caretPositionFromPoint) {
            const hit = document.caretPositionFromPoint(x, y);
            node = hit?.offsetNode;
            offset = hit?.offset;
        } else if (document.caretRangeFromPoint) {
            const hit = document.caretRangeFromPoint(x, y);
            node = hit?.startContainer;
            offset = hit?.startOffset;
        }
        const text = line.textContent || '';
        let column;
        if (node && line.contains(node)) {
            const range = document.createRange();
            range.selectNodeContents(line);
            range.setEnd(node, offset);
            column = range.toString().length;
        } else {
            const box = line.getBoundingClientRect();
            column = x <= box.left + parseFloat(getComputedStyle(line).paddingLeft) ? 0 : text.length;
        }
        // A text position must not divide a UTF-16 surrogate pair.
        if (column > 0 && /[\uDC00-\uDFFF]/.test(text[column] || '')
            && /[\uD800-\uDBFF]/.test(text[column - 1])) column--;
        return column;
    };

    const sendPoint = (event, start) => {
        const line = event.target.closest?.('.virtual-text-line-view');
        if (!line || !root.contains(line)) return false;
        output.value = JSON.stringify({
            line_index: Number(line.dataset.lineIndex),
            column_utf16: hitColumn(line, event.clientX, event.clientY),
            start,
            extend: event.shiftKey,
        });
        output.dispatchEvent(new Event('input', { bubbles: true }));
        return true;
    };

    root.addEventListener('mousedown', event => {
        if (event.button !== 0 || !event.target.closest?.('.virtual-text-line-view')) return;
        event.preventDefault();
        dragging = sendPoint(event, true);
        const input = root.querySelector('#rton-virtual-text-input-sink');
        if (input) {
            input.value = '';
            input.focus({ preventScroll: true });
            input.setSelectionRange(0, 0);
        }
    });
    root.addEventListener('mousemove', event => {
        if (dragging && (event.buttons & 1)) sendPoint(event, false);
    });
    root.addEventListener('mouseup', () => { dragging = false; });
    root.addEventListener('mouseleave', () => { dragging = false; });
})();
