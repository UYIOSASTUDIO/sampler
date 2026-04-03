// src/lib/stores/scanner.svelte.ts
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import type { ScanProgressPayload } from '$lib/types';

export const scannerState = $state({
    queue: [] as string[],
    isSyncing: false,
    isScanningNew: false,
    isClearing: false,
    message: '',
    total: 0,
    current: 0,
    currentFile: ''
});

export function getIsScanning() {
    return scannerState.isSyncing || scannerState.isScanningNew || scannerState.isClearing;
}

export function getScanPercentage() {
    return scannerState.total > 0 ? Math.round((scannerState.current / scannerState.total) * 100) : 0;
}

export async function initScannerListener() {
    await listen<ScanProgressPayload>('scan-progress', (e) => {
        scannerState.total = e.payload.total;
        scannerState.current = e.payload.current;
        scannerState.currentFile = e.payload.current_file;
    });
}

export async function handleSelectFolder() {
    try {
        const result = await open({ directory: true, multiple: true });
        if (result) {
            let added = 0;
            const processPath = (path: string) => {
                if (!scannerState.queue.includes(path)) {
                    scannerState.queue.push(path);
                    added++;
                }
            };
            if (Array.isArray(result)) result.forEach(processPath);
            else processPath(result as string);

            if (added > 0) scannerState.message = `${added} folder(s) added to queue.`;
        }
    } catch (error) { console.error(error); }
}

export async function handleScan(onComplete: () => void) {
    if (scannerState.queue.length === 0) return;
    scannerState.isScanningNew = true;
    let totalAdded = 0;

    const queueToProcess = [...scannerState.queue];
    for (const path of queueToProcess) {
        scannerState.message = `Indexing ${path.split(/[/\\]/).pop()}...`;
        try {
            const count = await invoke<number>('scan_library', { path });
            totalAdded += count;
            scannerState.queue = scannerState.queue.filter(p => p !== path);
        } catch (error) { console.error(`Error scanning ${path}:`, error); }
    }

    scannerState.message = `Batch complete. Added ${totalAdded} files in total.`;
    onComplete();
    scannerState.isScanningNew = false;
    setTimeout(() => { if (!scannerState.isScanningNew) scannerState.message = ''; }, 4000);
}

export async function handleRescanAll(onComplete: () => void) {
    scannerState.isSyncing = true;
    scannerState.message = 'Syncing folders...';
    try {
        const removed = await invoke<number>('cleanup_database');
        const added = await invoke<number>('rescan_all_folders');
        scannerState.message = `Synced! Added: ${added}, Removed: ${removed}`;
        onComplete();
        setTimeout(() => { scannerState.message = ''; }, 3000);
    } catch (error) { scannerState.message = `Error: ${error}`; }
    finally { scannerState.isSyncing = false; }
}