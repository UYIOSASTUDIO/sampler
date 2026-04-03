// src/lib/stores/editor.svelte.ts
import { invoke } from '@tauri-apps/api/core';
import type { SampleRecord } from '$lib/types';

export const editorState = $state({
    isOpen: false,
    sample: null as SampleRecord | null,
    currentPreviewPath: null as string | null,
    exportSlicedPath: null as string | null,
    previewSliceStartPct: 0,
    isSlicing: false,
    isSliceReady: false,
    isLooping: false,
    trimStartPct: 0,
    trimEndPct: 1,
    isDraggingHandle: null as 'start' | 'end' | null,
    zoomLevel: 1.0,
    scrollContainer: undefined as HTMLDivElement | undefined,
    waveformContainer: undefined as HTMLDivElement | undefined,
});

export function openSampler(sample: SampleRecord) {
    editorState.zoomLevel = 1.0;
    editorState.sample = sample;
    editorState.isOpen = true;
    editorState.isSliceReady = false;
    editorState.trimStartPct = 0;
    editorState.trimEndPct = 1;
    editorState.currentPreviewPath = null;
    editorState.exportSlicedPath = null;
}

export function closeSampler() {
    editorState.isOpen = false;
    editorState.sample = null;
    editorState.currentPreviewPath = null;
    editorState.exportSlicedPath = null;
    editorState.isSliceReady = false;
}

export async function confirmSlice() {
    if (!editorState.sample) return;
    editorState.isSlicing = true;
    try {
        const startMs = editorState.trimStartPct * editorState.sample.duration_ms;
        const endMs = editorState.trimEndPct * editorState.sample.duration_ms;
        editorState.exportSlicedPath = await invoke<string>('slice_audio', {
            path: editorState.sample.original_path,
            startMs: startMs,
            endMs: endMs
        });
        editorState.isSliceReady = true;
    } catch (e) {
        console.error("Failed to slice audio:", e);
    } finally {
        editorState.isSlicing = false;
    }
}

export function editSlice() {
    editorState.isSliceReady = false;
    editorState.exportSlicedPath = null;
}