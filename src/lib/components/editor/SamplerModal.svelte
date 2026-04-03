<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { X, Download, RefreshCw, Repeat, ZoomIn, ZoomOut, Play, Pause, Image as ImageIcon } from 'lucide-svelte';
    import { appState } from '$lib/store.svelte';
    import { editorState, closeSampler, confirmSlice, editSlice } from '$lib/stores/editor.svelte';
    import { formatDuration, parseWaveform } from '$lib/utils/format';
    import { nativeSliceDrag } from '$lib/utils/drag';
    import { tick } from 'svelte';

    let { playSlicePreview }: { playSlicePreview: (forceRestart?: boolean) => void } = $props();

    function handleZoomIn() { editorState.zoomLevel = Math.min(10.0, editorState.zoomLevel * 1.5); }
    function handleZoomOut() { editorState.zoomLevel = Math.max(1.0, editorState.zoomLevel / 1.5); }

    function handleWaveformClick(e: MouseEvent) {
        if (editorState.isSliceReady || !editorState.waveformContainer) return;
        const rect = editorState.waveformContainer.getBoundingClientRect();
        let pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
        if (pct < 0.5) {
            editorState.trimStartPct = Math.min(pct, editorState.trimEndPct - 0.02);
            editorState.currentPreviewPath = null;
            if (appState.isPlaying) playSlicePreview(true);
        } else {
            editorState.trimEndPct = Math.max(pct, editorState.trimStartPct + 0.02);
        }
    }

    function editorWheelZoom(node: HTMLElement) {
        const handleWheel = (e: WheelEvent) => {
            if (e.ctrlKey || e.metaKey) {
                e.preventDefault();
                if (!editorState.scrollContainer || !editorState.waveformContainer) return;

                const zoomFactor = 1.1;
                const isZoomingIn = e.deltaY < 0;
                let newZoom = isZoomingIn ? editorState.zoomLevel * zoomFactor : editorState.zoomLevel / zoomFactor;
                newZoom = Math.max(1.0, Math.min(10.0, newZoom));

                if (newZoom === editorState.zoomLevel) return;

                const rect = editorState.waveformContainer.getBoundingClientRect();
                const mousePct = (e.clientX - rect.left) / rect.width;
                editorState.zoomLevel = newZoom;

                tick().then(() => {
                    if (!editorState.waveformContainer || !editorState.scrollContainer) return;
                    const newRect = editorState.waveformContainer.getBoundingClientRect();
                    const newMouseXRel = mousePct * newRect.width;
                    const scrollOffset = e.clientX - editorState.scrollContainer.getBoundingClientRect().left;
                    editorState.scrollContainer.scrollLeft = newMouseXRel - scrollOffset;
                });
            }
        };
        node.addEventListener('wheel', handleWheel, { passive: false });
        return { destroy() { node.removeEventListener('wheel', handleWheel); } };
    }

    function handleEditorMouseMove(e: MouseEvent) {
        if (!editorState.isDraggingHandle || !editorState.waveformContainer || editorState.isSliceReady) return;
        const rect = editorState.waveformContainer.getBoundingClientRect();
        let pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));

        if (editorState.isDraggingHandle === 'start') {
            editorState.trimStartPct = Math.min(pct, editorState.trimEndPct - 0.02);
            editorState.currentPreviewPath = null;
        } else {
            editorState.trimEndPct = Math.max(pct, editorState.trimStartPct + 0.02);
        }
    }

    function handleEditorMouseUp() {
        if (editorState.isDraggingHandle) {
            const wasStart = editorState.isDraggingHandle === 'start';
            editorState.isDraggingHandle = null;
            if (wasStart && appState.isPlaying && !editorState.isSliceReady) {
                playSlicePreview(true);
            }
        }
    }
</script>

<svelte:window onmousemove={handleEditorMouseMove} onmouseup={handleEditorMouseUp} />

{#if editorState.sample}
    <div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/60 backdrop-blur-md animate-in fade-in duration-200 p-8">
        <div class="w-full max-w-5xl bg-white dark:bg-[#18181b] border border-zinc-200 dark:border-zinc-800 rounded-2xl shadow-2xl flex flex-col overflow-hidden">
            <div class="flex items-center justify-between px-6 py-4 border-b border-zinc-100 dark:border-zinc-800/50 bg-zinc-50/50 dark:bg-zinc-900/50">
                <div class="flex items-center gap-4">
                    <div class="h-12 w-12 flex items-center justify-center rounded-md bg-zinc-200 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700 overflow-hidden shrink-0 shadow-sm">
                        {#if editorState.sample.cover_path}
                            <img src={convertFileSrc(editorState.sample.cover_path)} alt="Cover" class="h-full w-full object-cover" />
                        {:else}
                            <ImageIcon size={20} class="text-zinc-400" />
                        {/if}
                    </div>
                    <div>
                        <h2 class="text-lg font-bold text-zinc-900 dark:text-white leading-none">{editorState.sample.filename}</h2>
                        <div class="flex items-center gap-2 mt-2 text-[11px] font-semibold text-zinc-500 uppercase tracking-wider">
                            <span class="bg-zinc-200/60 dark:bg-zinc-800 px-1.5 py-[2px] rounded text-zinc-700 dark:text-zinc-300">WAV</span>
                            <span>{formatDuration(editorState.sample.duration_ms)}</span>
                            {#if editorState.sample.bpm}<span>• {Math.round(editorState.sample.bpm)} BPM</span>{/if}
                            {#if editorState.sample.key_signature}<span>• {editorState.sample.key_signature}</span>{/if}
                        </div>
                    </div>
                </div>
                <button onclick={closeSampler} class="h-8 w-8 flex items-center justify-center rounded-full text-zinc-400 hover:bg-zinc-200 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-white transition-colors cursor-pointer">
                    <X size={20} />
                </button>
            </div>

            <div class="p-6 pb-8">
                <div class="flex justify-between mb-3 text-[11px] font-bold uppercase tracking-wider text-zinc-400 tabular-nums px-1">
                    <span>Start: {formatDuration(editorState.sample.duration_ms * editorState.trimStartPct)}</span>
                    <span class="text-emerald-600 dark:text-emerald-400 bg-emerald-50 dark:bg-emerald-900/20 px-2 py-0.5 rounded border border-emerald-200 dark:border-emerald-800/50">
                    Selection Length: {formatDuration(editorState.sample.duration_ms * (editorState.trimEndPct - editorState.trimStartPct))}
                </span>
                    <span>End: {formatDuration(editorState.sample.duration_ms * editorState.trimEndPct)}</span>
                </div>

                <div bind:this={editorState.scrollContainer} use:editorWheelZoom class="w-full h-48 bg-zinc-100 dark:bg-[#121214] border border-zinc-200 dark:border-zinc-800 rounded-xl {editorState.zoomLevel > 1.0 ? 'overflow-x-auto' : 'overflow-hidden'} overflow-y-hidden shadow-inner relative no-scrollbar">
                    <div bind:this={editorState.waveformContainer} class="relative h-full select-none origin-left" style="width: {editorState.zoomLevel * 100}%">
                        {#if !editorState.isSliceReady}
                            <div class="absolute inset-0 z-20 cursor-crosshair" onmousedown={handleWaveformClick} role="button" tabindex="0"></div>
                        {/if}

                        <div class="absolute inset-0 flex items-center justify-between gap-[1px] opacity-20 pointer-events-none">
                            {#each parseWaveform(editorState.sample.waveform_data, Math.floor(300 * editorState.zoomLevel)) as barHeight}
                                <div class="w-full rounded-full bg-zinc-500" style="height: {barHeight}%;"></div>
                            {/each}
                        </div>

                        <div class="absolute inset-0 flex items-center justify-between gap-[1px] pointer-events-none" style="clip-path: polygon({editorState.trimStartPct * 100}% 0, {editorState.trimEndPct * 100}% 0, {editorState.trimEndPct * 100}% 100%, {editorState.trimStartPct * 100}% 100%);">
                            {#each parseWaveform(editorState.sample.waveform_data, Math.floor(300 * editorState.zoomLevel)) as barHeight}
                                <div class="w-full rounded-full {editorState.isSliceReady ? 'bg-emerald-400' : 'bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.4)]'} transition-colors" style="height: {barHeight}%;"></div>
                            {/each}
                        </div>

                        <div class="absolute top-0 bottom-0 {editorState.isSliceReady ? 'bg-emerald-500/10' : 'bg-transparent'} transition-colors duration-300 pointer-events-none" style="left: {editorState.trimStartPct * 100}%; right: {(1 - editorState.trimEndPct) * 100}%">
                            <div class="absolute -top-3 left-1/2 -translate-x-1/2 opacity-0 group-hover:opacity-100 transition-opacity mt-6 z-30 pointer-events-none">
                                <div class="bg-zinc-900 dark:bg-black text-white text-[10px] font-bold px-3 py-1.5 rounded-full shadow-xl flex items-center gap-1.5 border border-zinc-700/50">
                                    {#if editorState.isSlicing} <RefreshCw size={12} class="animate-spin text-emerald-400" /> <span>Slicing...</span>
                                    {:else} <Download size={12} class="text-emerald-400" /> <span>Drag to DAW</span> {/if}
                                </div>
                            </div>

                            {#if appState.isPlaying && editorState.isOpen}
                                <div class="absolute top-0 bottom-0 w-0.5 bg-white shadow-[0_0_10px_rgba(255,255,255,1)] z-50 pointer-events-none" style="left: {appState.playbackProgress * 100}%"></div>
                            {/if}

                            {#if editorState.isSliceReady}
                                <div use:nativeSliceDrag={{ isSliceReady: editorState.isSliceReady, exportSlicedPath: editorState.exportSlicedPath, samplerSample: editorState.sample }} class="absolute inset-0 flex items-center justify-center opacity-0 hover:opacity-100 transition-opacity bg-emerald-900/40 cursor-grab active:cursor-grabbing backdrop-blur-sm z-30 pointer-events-auto">
                                    <span class="text-white font-bold tracking-wider uppercase text-sm px-4 py-2 bg-black/80 shadow-xl rounded-md flex items-center gap-2"><Download size={16} /> Drag to DAW</span>
                                </div>
                            {:else}
                                <div class="absolute left-0 top-0 bottom-0 w-6 cursor-ew-resize flex flex-col items-start group/handle z-40 pointer-events-auto" onmousedown={(e) => { editorState.isDraggingHandle = 'start'; e.stopPropagation(); }}>
                                    <div class="bg-emerald-600 group-hover/handle:bg-emerald-400 text-white text-[9px] font-black px-1.5 py-0.5 rounded-br-md shadow-md transition-colors">S</div>
                                    <div class="w-[2px] h-full bg-emerald-600/80 group-hover/handle:bg-emerald-400 shadow-sm transition-colors -mt-[1px]"></div>
                                </div>
                                <div class="absolute right-0 top-0 bottom-0 w-6 cursor-ew-resize flex flex-col items-end group/handle z-40 pointer-events-auto" onmousedown={(e) => { editorState.isDraggingHandle = 'end'; e.stopPropagation(); }}>
                                    <div class="bg-emerald-600 group-hover/handle:bg-emerald-400 text-white text-[9px] font-black px-1.5 py-0.5 rounded-bl-md shadow-md transition-colors">E</div>
                                    <div class="w-[2px] h-full bg-emerald-600/80 group-hover/handle:bg-emerald-400 shadow-sm transition-colors -mt-[1px]"></div>
                                </div>
                            {/if}
                        </div>
                    </div>
                </div>

                <div class="mt-5 flex items-center justify-between">
                    <div class="flex items-center gap-4">
                        <button onclick={() => editorState.isLooping = !editorState.isLooping} class="flex items-center gap-2 px-3 py-2 rounded-md text-[11px] font-bold uppercase tracking-wider transition-colors {editorState.isLooping ? 'bg-emerald-500 text-white shadow-sm' : 'bg-zinc-100 text-zinc-600 hover:bg-zinc-200 dark:bg-zinc-800/50 dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-white'} cursor-pointer">
                            <Repeat size={14} /> Loop
                        </button>
                        <div class="flex items-center rounded-md border border-zinc-200 dark:border-zinc-800/60 bg-white dark:bg-[#18181b] shadow-sm">
                            <button onclick={handleZoomOut} disabled={editorState.zoomLevel <= 1.0} class="flex items-center justify-center h-8 w-8 text-zinc-500 hover:text-zinc-900 hover:bg-zinc-50 dark:hover:bg-zinc-800 dark:hover:text-zinc-100 disabled:opacity-30 transition-colors rounded-l-md cursor-pointer"><ZoomOut size={14} /></button>
                            <div class="w-px h-4 bg-zinc-200 dark:bg-zinc-800"></div>
                            <span class="text-[10px] font-bold text-zinc-500 w-12 text-center tabular-nums cursor-default select-none">{Math.round(editorState.zoomLevel * 100)}%</span>
                            <div class="w-px h-4 bg-zinc-200 dark:bg-zinc-800"></div>
                            <button onclick={handleZoomIn} disabled={editorState.zoomLevel >= 20.0} class="flex items-center justify-center h-8 w-8 text-zinc-500 hover:text-zinc-900 hover:bg-zinc-50 dark:hover:bg-zinc-800 dark:hover:text-zinc-100 disabled:opacity-30 transition-colors rounded-r-md cursor-pointer"><ZoomIn size={14} /></button>
                        </div>
                    </div>

                    <div class="flex items-center gap-3">
                        <button onclick={() => playSlicePreview()} class="flex items-center gap-2 px-4 py-2 rounded-md text-xs font-bold uppercase tracking-wider transition-colors {appState.isPlaying ? 'bg-zinc-200 text-zinc-900 dark:bg-zinc-800 dark:text-white' : 'bg-zinc-100 text-zinc-600 hover:bg-zinc-200 dark:bg-zinc-800/50 dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-white'} cursor-pointer">
                            {#if appState.isPlaying} <Pause size={14}/> Stop {:else} <Play size={14}/> Preview (Space) {/if}
                        </button>
                        {#if !editorState.isSliceReady}
                            <button onclick={confirmSlice} disabled={editorState.isSlicing} class="flex items-center gap-2 px-5 py-2 rounded-md text-xs font-bold uppercase tracking-wider bg-emerald-500 text-white hover:bg-emerald-600 transition-colors shadow-sm disabled:opacity-50 cursor-pointer">
                                {#if editorState.isSlicing} <RefreshCw size={14} class="animate-spin"/> Rendering... {:else} Confirm Selection {/if}
                            </button>
                        {:else}
                            <button onclick={editSlice} class="flex items-center gap-2 px-5 py-2 rounded-md text-xs font-bold uppercase tracking-wider bg-zinc-900 text-white hover:bg-zinc-800 dark:bg-white dark:text-zinc-900 dark:hover:bg-zinc-200 transition-colors shadow-sm cursor-pointer">
                                Edit Selection
                            </button>
                        {/if}
                    </div>
                </div>
            </div>
        </div>
    </div>
{/if}