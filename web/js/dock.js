/**
 * Dockable side-menu: drag the grip handle to dock left/right/top/bottom.
 * Persists the chosen dock side in localStorage.
 */
(function () {
    const menu   = document.getElementById('side-menu');
    const handle = document.getElementById('dock-handle');
    const toggle = document.getElementById('menu-toggle');
    const previews = document.querySelectorAll('.dock-preview');

    const EDGE_THRESHOLD = 80; // px from viewport edge to trigger dock zone
    const STORAGE_KEY = 'rugb-dock-side';

    /* ---- Menu toggle (click to open/close) ---- */
    toggle.addEventListener('click', () => menu.classList.toggle('open'));

    /* ---- Restore saved dock ---- */
    const saved = localStorage.getItem(STORAGE_KEY) || 'left';
    applyDock(saved);

    /* ---- Drag state ---- */
    let dragging = false;
    let startX, startY;

    handle.addEventListener('pointerdown', onPointerDown);

    function onPointerDown(e) {
        // Only primary button
        if (e.button !== 0) return;
        e.preventDefault();
        dragging = true;
        startX = e.clientX;
        startY = e.clientY;
        handle.setPointerCapture(e.pointerId);
        // Disable transition while dragging
        menu.style.transition = 'none';
        document.addEventListener('pointermove', onPointerMove);
        document.addEventListener('pointerup', onPointerUp);
    }

    function onPointerMove(e) {
        if (!dragging) return;
        const zone = detectZone(e.clientX, e.clientY);
        previews.forEach(p => {
            p.classList.toggle('visible', p.dataset.zone === zone);
        });
    }

    function onPointerUp(e) {
        if (!dragging) return;
        dragging = false;
        document.removeEventListener('pointermove', onPointerMove);
        document.removeEventListener('pointerup', onPointerUp);
        // Hide all previews
        previews.forEach(p => p.classList.remove('visible'));

        // If the pointer barely moved, treat it as a click (don't change dock)
        const dx = Math.abs(e.clientX - startX);
        const dy = Math.abs(e.clientY - startY);
        if (dx < 8 && dy < 8) {
            menu.style.transition = '';
            return;
        }

        const zone = detectZone(e.clientX, e.clientY);
        if (zone) {
            const wasOpen = menu.classList.contains('open');
            menu.classList.remove('open');
            applyDock(zone);
            localStorage.setItem(STORAGE_KEY, zone);
            if (wasOpen) {
                // Re-open with transition after dock change
                requestAnimationFrame(() => {
                    menu.style.transition = '';
                    requestAnimationFrame(() => menu.classList.add('open'));
                });
            } else {
                menu.style.transition = '';
            }
        } else {
            menu.style.transition = '';
        }
    }

    function detectZone(x, y) {
        const vw = window.innerWidth;
        const vh = window.innerHeight;
        // Check edges — corners prefer horizontal (top/bottom)
        if (y < EDGE_THRESHOLD) return 'top';
        if (y > vh - EDGE_THRESHOLD) return 'bottom';
        if (x < EDGE_THRESHOLD) return 'left';
        if (x > vw - EDGE_THRESHOLD) return 'right';
        return null;
    }

    function applyDock(side) {
        // Clear inline position styles
        menu.style.left = '';
        menu.style.right = '';
        menu.style.top = '';
        menu.style.bottom = '';
        menu.style.width = '';
        menu.style.height = '';
        menu.style.borderLeft = '';
        menu.style.borderRight = '';
        menu.style.borderTop = '';
        menu.style.borderBottom = '';

        menu.dataset.dock = side;
        positionToggle(side);
    }

    function positionToggle(side) {
        // Move the hamburger button to match the dock side
        toggle.style.top = '';
        toggle.style.bottom = '';
        toggle.style.left = '';
        toggle.style.right = '';
        switch (side) {
            case 'left':
                toggle.style.top = '12px';
                toggle.style.left = '12px';
                break;
            case 'right':
                toggle.style.top = '12px';
                toggle.style.right = '12px';
                break;
            case 'top':
                toggle.style.top = '12px';
                toggle.style.left = '12px';
                break;
            case 'bottom':
                toggle.style.bottom = '12px';
                toggle.style.left = '12px';
                break;
        }
    }
})();
