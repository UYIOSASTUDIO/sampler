// src/lib/utils/window.ts
import { getCurrentWindow } from '@tauri-apps/api/window';

export function dragRegion(node: HTMLElement) {
    let lastClickTime = 0;

    const handleMouseDown = async (e: MouseEvent) => {
        const target = e.target as HTMLElement;

        if (target.tagName === 'INPUT' || target.tagName === 'BUTTON' || target.closest('button')) {
            return;
        }

        if (e.buttons === 1) {
            const currentTime = Date.now();
            const timeDiff = currentTime - lastClickTime;
            lastClickTime = currentTime;

            if (timeDiff < 400) {
                try {
                    await getCurrentWindow().toggleMaximize();
                    return;
                } catch(err) {
                    console.error("Failed to toggle maximize:", err);
                }
            }

            try {
                getCurrentWindow().startDragging();
            } catch(err) {
                console.error("Failed to drag window:", err);
            }
        }
    };

    node.addEventListener('mousedown', handleMouseDown);

    return {
        destroy() {
            node.removeEventListener('mousedown', handleMouseDown);
        }
    };
}