// src/lib/utils/drag.ts
import { convertFileSrc } from '@tauri-apps/api/core';
import { appDataDir } from '@tauri-apps/api/path';
import { writeFile, mkdir } from '@tauri-apps/plugin-fs';
import { startDrag } from '@crabnebula/tauri-plugin-drag';
import type { SampleRecord } from '$lib/types';

let appDataPath = '';
const dragIconCache = new Map<string, string>();

export async function initDragDrop() {
    try {
        const dir = await appDataDir();
        appDataPath = dir.endsWith('/') || dir.endsWith('\\') ? dir : dir + '/';
        await mkdir(appDataPath + 'drag-icons', { recursive: true });
    } catch (e) {
        console.warn('[SampleVault] appDataDir nicht aufgelöst:', e);
    }
}

export async function prepareDragIcon(sample: SampleRecord): Promise<string> {
    if (dragIconCache.has(sample.id)) return dragIconCache.get(sample.id)!;
    if (!appDataPath) return '';

    const SIZE = 64;
    const canvas = document.createElement('canvas');
    canvas.width = SIZE; canvas.height = SIZE;
    const ctx = canvas.getContext('2d')!;

    let drewCover = false;
    if (sample.cover_path) {
        try {
            const response = await fetch(convertFileSrc(sample.cover_path));
            if (response.ok) {
                const imgBlob = await response.blob();
                const objectUrl = URL.createObjectURL(imgBlob);
                const img = new Image();
                await new Promise<void>((resolve) => {
                    img.onload = () => resolve();
                    img.onerror = () => resolve();
                    img.src = objectUrl;
                });
                URL.revokeObjectURL(objectUrl);
                if (img.naturalWidth > 0) {
                    ctx.save();
                    ctx.beginPath();
                    if (ctx.roundRect) ctx.roundRect(0, 0, SIZE, SIZE, 10);
                    else ctx.rect(0, 0, SIZE, SIZE);
                    ctx.clip();
                    ctx.drawImage(img, 0, 0, SIZE, SIZE);
                    ctx.restore();
                    drewCover = true;
                }
            }
        } catch { /* Fehler → Fallback-Icon */ }
    }

    if (!drewCover) {
        ctx.fillStyle = '#27272a';
        ctx.beginPath();
        if (ctx.roundRect) ctx.roundRect(0, 0, SIZE, SIZE, 10);
        else ctx.rect(0, 0, SIZE, SIZE);
        ctx.fill();
        ctx.fillStyle = '#22c55e';
        ctx.font = 'bold 38px serif';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText('♪', SIZE / 2, SIZE / 2 + 2);
    }

    const blob = await new Promise<Blob>((resolve) => canvas.toBlob((b) => resolve(b!), 'image/png'));
    const bytes = new Uint8Array(await blob.arrayBuffer());
    const iconPath = appDataPath + 'drag-icons/' + sample.id + '.png';

    await writeFile(iconPath, bytes);
    dragIconCache.set(sample.id, iconPath);
    return iconPath;
}

export function nativeDrag(node: HTMLElement, sampleArg: SampleRecord) {
    let sample = sampleArg;
    let startX = 0; let startY = 0;
    let isDragging = false; let didDrag = false;
    let iconPromise: Promise<string> | null = null;

    const handleMouseMove = async (e: MouseEvent) => {
        if (isDragging) return;
        const dx = Math.abs(e.clientX - startX);
        const dy = Math.abs(e.clientY - startY);

        if (dx > 5 || dy > 5) {
            isDragging = true;
            cleanupWindowListeners();
            try {
                const icon = await (iconPromise ?? prepareDragIcon(sample));
                await startDrag({ item: [sample.original_path], icon });
                didDrag = true;
            } catch (err) { console.error('[SampleVault] OS-Drag fehlgeschlagen:', err); }
            finally { isDragging = false; }
        }
    };

    const handleMouseUp = () => cleanupWindowListeners();
    const handleClick = (e: MouseEvent) => { if (didDrag) { e.stopPropagation(); e.preventDefault(); didDrag = false; } };
    const cleanupWindowListeners = () => { window.removeEventListener('mousemove', handleMouseMove); window.removeEventListener('mouseup', handleMouseUp); };

    const handleMouseDown = (e: MouseEvent) => {
        if (e.button !== 0) return;
        e.preventDefault();
        startX = e.clientX; startY = e.clientY;
        isDragging = false; didDrag = false;
        iconPromise = prepareDragIcon(sample).catch(() => '');
        window.addEventListener('mousemove', handleMouseMove);
        window.addEventListener('mouseup', handleMouseUp);
    };

    node.addEventListener('mousedown', handleMouseDown);
    node.addEventListener('click', handleClick);

    return {
        update(newSample: SampleRecord) { sample = newSample; iconPromise = null; },
        destroy() { node.removeEventListener('mousedown', handleMouseDown); node.removeEventListener('click', handleClick); cleanupWindowListeners(); }
    };
}

export function nativeSliceDrag(node: HTMLElement, params: { isSliceReady: boolean, exportSlicedPath: string | null, samplerSample: SampleRecord | null }) {
    let { isSliceReady, exportSlicedPath, samplerSample } = params;
    let startX = 0; let startY = 0;
    let isDragging = false; let didDrag = false;

    const handleMouseMove = async (e: MouseEvent) => {
        if (isDragging || !isSliceReady || !exportSlicedPath || !samplerSample) return;
        const dx = Math.abs(e.clientX - startX);
        const dy = Math.abs(e.clientY - startY);

        if (dx > 5 || dy > 5) {
            isDragging = true;
            cleanupWindowListeners();
            try {
                const icon = await prepareDragIcon(samplerSample).catch(() => '');
                await startDrag({ item: [exportSlicedPath], icon });
                didDrag = true;
            } catch (err) { console.error("Slice & Drag Error:", err); }
            finally { isDragging = false; }
        }
    };

    const handleMouseUp = () => cleanupWindowListeners();
    const handleClick = (e: MouseEvent) => { if (didDrag) { e.stopPropagation(); e.preventDefault(); didDrag = false; } };
    const cleanupWindowListeners = () => { window.removeEventListener('mousemove', handleMouseMove); window.removeEventListener('mouseup', handleMouseUp); };

    const handleMouseDown = (e: MouseEvent) => {
        if (e.button !== 0 || !isSliceReady) return;
        e.preventDefault();
        startX = e.clientX; startY = e.clientY;
        isDragging = false; didDrag = false;
        window.addEventListener('mousemove', handleMouseMove);
        window.addEventListener('mouseup', handleMouseUp);
    };

    node.addEventListener('mousedown', handleMouseDown);
    node.addEventListener('click', handleClick);

    return {
        update(newParams: { isSliceReady: boolean, exportSlicedPath: string | null, samplerSample: SampleRecord | null }) {
            isSliceReady = newParams.isSliceReady;
            exportSlicedPath = newParams.exportSlicedPath;
            samplerSample = newParams.samplerSample;
        },
        destroy() { node.removeEventListener('mousedown', handleMouseDown); node.removeEventListener('click', handleClick); cleanupWindowListeners(); }
    };
}