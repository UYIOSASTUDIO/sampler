<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { open } from '@tauri-apps/plugin-dialog';
    import { EllipsisVertical, Download, Heart, Play, Pause, FolderPlus, RefreshCw, Trash2, Image as ImageIcon, ChevronLeft, ChevronRight } from 'lucide-svelte';

    type SampleRecord = {
        id: string;
        filename: string;
        original_path: string;
        duration_ms: number;
        bpm: number | null;
        key_signature: string | null;
        instrument_type: string | null;
        waveform_data: string | null;
    };

    type PaginatedResponse = {
        samples: SampleRecord[];
        total_count: number;
    };

    let samples: SampleRecord[] = $state([]);
    let isLoading: boolean = $state(true);

    let selectedPath: string | null = $state(null);
    let isScanning: boolean = $state(false);
    let scanMessage: string = $state('');
    let activeTypeFilter: string | null = $state(null);

    let scrollContainer: HTMLDivElement;

    let currentPage: number = $state(1);
    const pageSize: number = 50;
    let totalItems: number = $state(0);
    let totalPages: number = $derived(Math.ceil(totalItems / pageSize) || 1);

    let visiblePages = $derived.by(() => {
        let pages = [];
        if (totalPages <= 5) {
            for (let i = 1; i <= totalPages; i++) pages.push(i);
        } else {
            if (currentPage <= 3) {
                pages = [1, 2, 3, 4, 5];
            } else if (currentPage >= totalPages - 2) {
                pages = [totalPages - 4, totalPages - 3, totalPages - 2, totalPages - 1, totalPages];
            } else {
                pages = [currentPage - 2, currentPage - 1, currentPage, currentPage + 1, currentPage + 2];
            }
        }
        return pages;
    });

    // --- BULLETPROOF AUDIO ENGINE & KEYBOARD NAV ---
    let globalAudio: HTMLAudioElement;
    let playingId: string | null = $state(null);
    let selectedId: string | null = $state(null); // Speichert die aktive Zeile
    let currentBlobUrl: string | null = null;
    let playbackProgress: number = $state(0);

    let animationFrameId: number;
    let currentPlayRequest: number = 0; // Token-System für Race Conditions

    const filters = ["Instruments", "Genres", "Key", "BPM", "One-Shots & Loops"];
    const instrumentTypes = ["Kick", "Snare", "Clap", "HiHat", "Cymbal", "Percussion", "Bass", "Vocal", "FX", "Synth", "Loop"];

    // 60fps Render-Loop für die Waveform-Balken
    function updateProgress() {
        if (globalAudio && !globalAudio.paused && globalAudio.duration) {
            playbackProgress = globalAudio.currentTime / globalAudio.duration;
            animationFrameId = requestAnimationFrame(updateProgress);
        }
    }

    onMount(async () => {
        globalAudio = new window.Audio();
        globalAudio.loop = false;

        globalAudio.onplay = () => {
            cancelAnimationFrame(animationFrameId);
            animationFrameId = requestAnimationFrame(updateProgress);
        };

        globalAudio.onpause = () => {
            cancelAnimationFrame(animationFrameId);
        };

        globalAudio.onended = () => {
            playingId = null;
            playbackProgress = 0;
            cancelAnimationFrame(animationFrameId);
        };

        window.addEventListener('keydown', handleKeydown);
        await loadSamples();
    });

    onDestroy(() => {
        if (globalAudio) globalAudio.pause();
        if (currentBlobUrl) URL.revokeObjectURL(currentBlobUrl);
        cancelAnimationFrame(animationFrameId);
        window.removeEventListener('keydown', handleKeydown);
    });

    async function loadSamples() {
        isLoading = true;
        try {
            const response = await invoke<PaginatedResponse>('get_samples', { filterType: activeTypeFilter, page: currentPage, pageSize: pageSize });
            samples = response.samples;
            totalItems = response.total_count;
            if (scrollContainer) scrollContainer.scrollTop = 0;
            if (globalAudio && !globalAudio.paused) {
                globalAudio.pause();
                playingId = null;
            }
        } catch (error) {
            console.error("Failed to load samples", error);
        } finally {
            isLoading = false;
        }
    }

    // --- KEYBOARD LOGIC ---
    function handleKeydown(e: KeyboardEvent) {
        // Ignoriere Tastatur, wenn der User in einem Textfeld tippt
        if (document.activeElement?.tagName === 'INPUT') return;

        if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault();
            if (samples.length === 0) return;

            let currentIndex = samples.findIndex(s => s.id === selectedId);

            if (e.key === 'ArrowDown') {
                currentIndex = currentIndex === -1 ? 0 : Math.min(samples.length - 1, currentIndex + 1);
            } else {
                currentIndex = currentIndex === -1 ? 0 : Math.max(0, currentIndex - 1);
            }

            const nextSample = samples[currentIndex];
            handlePlayRequest(nextSample);
        }
    }

    // --- AUDIO PLAY LOGIC ---
    async function handlePlayRequest(sample: SampleRecord) {
        const requestId = ++currentPlayRequest;
        selectedId = sample.id;

        if (playingId === sample.id) {
            globalAudio.pause();
            playingId = null;
            return;
        }

        // UI direkt aktualisieren, bevor der asynchrone Prozess startet
        playingId = sample.id;
        playbackProgress = 0;
        cancelAnimationFrame(animationFrameId);

        globalAudio.pause();
        if (currentBlobUrl) URL.revokeObjectURL(currentBlobUrl);

        try {
            const bytes = await invoke<number[]>('read_audio_file', { path: sample.original_path });

            // Race Condition Abwehr: Verwerfen, falls ein neuer Request gestartet wurde
            if (requestId !== currentPlayRequest) return;

            const blob = new Blob([new Uint8Array(bytes)]);
            currentBlobUrl = URL.createObjectURL(blob);
            globalAudio.src = currentBlobUrl;
            await globalAudio.play();
        } catch (error) {
            if (requestId === currentPlayRequest) {
                console.error("Failed to play audio:", error);
                playingId = null;
            }
        }
    }

    // ... Pagination & UI Helper (nextPage, prevPage, toggleFilter, parseWaveform, etc.) bleiben gleich ...
    function goToPage(p: number) { if (p !== currentPage) { currentPage = p; loadSamples(); } }
    function nextPage() { if (currentPage < totalPages) { currentPage++; loadSamples(); } }
    function prevPage() { if (currentPage > 1) { currentPage--; loadSamples(); } }
    function toggleFilter(type: string) { activeTypeFilter = activeTypeFilter === type ? null : type; currentPage = 1; loadSamples(); }
    function parseWaveform(data: string | null): number[] { if (!data) return Array(40).fill(10); try { return JSON.parse(data); } catch { return Array(40).fill(10); } }

    async function handleSelectFolder() { try { const result = await open({ directory: true, multiple: false }); if (result) { selectedPath = result as string; scanMessage = 'Ready to scan.'; } } catch (error) { console.error(error); } }
    async function handleScan() { if (!selectedPath) return; isScanning = true; scanMessage = 'Indexing...'; try { const count = await invoke<number>('scan_library', { path: selectedPath }); scanMessage = `Added ${count} files.`; selectedPath = null; currentPage = 1; await loadSamples(); } catch (error) { scanMessage = `Error: ${error}`; } finally { isScanning = false; } }
    async function handleClearDatabase() { if (confirm("Clear the entire library?")) { isScanning = true; try { await invoke('clear_database'); activeTypeFilter = null; currentPage = 1; samples = []; totalItems = 0; scanMessage = 'Library cleared.'; } catch (error) { scanMessage = `Error: ${error}`; } finally { isScanning = false; } } }
    function formatDuration(ms: number): string { if (ms === 0) return "--:--"; const totalSec = Math.floor(ms / 1000); return `${Math.floor(totalSec / 60)}:${(totalSec % 60).toString().padStart(2, '0')}`; }
</script>

<div class="flex flex-col h-full overflow-hidden">
    <div class="px-8 pt-8 shrink-0">
        <div class="mb-6 flex items-end justify-between border-b border-zinc-200 pb-0 dark:border-zinc-800">
            <div>
                <h1 class="text-3xl font-bold tracking-tight mb-4">Sounds</h1>
                <div class="flex gap-6">
                    <button class="border-b-2 border-zinc-900 pb-2 text-sm font-semibold text-zinc-900 dark:border-zinc-100 dark:text-zinc-100">Samples</button>
                    <button class="pb-2 text-sm font-medium text-zinc-500 hover:text-zinc-900 dark:text-zinc-400">Presets</button>
                    <button class="pb-2 text-sm font-medium text-zinc-500 hover:text-zinc-900 dark:text-zinc-400">Packs</button>
                </div>
            </div>

            <div class="flex flex-col items-end gap-3 pb-2">
                {#if scanMessage} <span class="text-xs font-medium text-zinc-500 animate-pulse">{scanMessage}</span> {/if}
                <div class="flex items-center gap-2">
                    <button onclick={handleSelectFolder} disabled={isScanning} class="flex items-center gap-1.5 rounded-md border border-zinc-200 bg-white px-3 py-1.5 text-xs font-medium hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-800 dark:hover:bg-zinc-700 transition-colors disabled:opacity-50 cursor-pointer">
                        <FolderPlus size={14} /> Browse
                    </button>
                    {#if selectedPath}
                        <button onclick={handleScan} disabled={isScanning} class="flex items-center gap-1.5 rounded-md border border-emerald-700/50 bg-emerald-50 px-3 py-1.5 text-xs font-medium text-emerald-700 hover:bg-emerald-100 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-400 cursor-pointer">
                            <RefreshCw size={14} class={isScanning ? "animate-spin" : ""} /> Start Scan
                        </button>
                    {/if}
                    <button onclick={handleClearDatabase} disabled={isScanning} class="flex items-center gap-1.5 rounded-md border border-red-200 bg-red-50 px-3 py-1.5 text-xs font-medium text-red-700 hover:bg-red-100 dark:border-red-900/50 dark:bg-red-950/30 dark:text-red-400 cursor-pointer ml-2">
                        <Trash2 size={14} /> Clear
                    </button>
                </div>
            </div>
        </div>

        <div class="mb-4 space-y-4">
            <div class="flex flex-wrap gap-2">
                {#each filters as filter}
                    <button class="flex items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 py-1.5 text-xs font-medium hover:bg-zinc-50 dark:border-zinc-700/50 dark:bg-zinc-800/30 dark:hover:bg-zinc-800 transition-colors">
                        {filter} <span class="text-[10px] opacity-50">▼</span>
                    </button>
                {/each}
            </div>
            <div class="flex flex-wrap gap-2">
                {#each instrumentTypes as type}
                    <button onclick={() => toggleFilter(type)} class="rounded-full border px-3 py-1 text-[11px] font-medium transition-colors cursor-pointer {activeTypeFilter === type ? 'bg-zinc-900 border-zinc-900 text-white dark:bg-zinc-100 dark:border-zinc-100 dark:text-zinc-900' : 'bg-zinc-50 border-zinc-200 text-zinc-600 hover:border-zinc-300 dark:bg-[#18181b] dark:border-zinc-800 dark:text-zinc-400'}">
                        {type}
                    </button>
                {/each}
            </div>
        </div>

        <div class="mb-2 flex items-center justify-between text-sm text-zinc-500">
            <span>{#if isLoading}Loading...{:else}{totalItems} results{/if}</span>
            <div class="flex items-center gap-2">
                <span class="text-xs font-medium">Sort by:</span>
                <button class="text-xs font-semibold text-zinc-900 dark:text-zinc-100">Most recent ▼</button>
            </div>
        </div>

        <div class="grid grid-cols-[20px_40px_32px_minmax(150px,2fr)_minmax(120px,1.5fr)_50px_40px_40px_32px_32px] gap-4 border-b border-zinc-200 pb-2 text-[11px] font-semibold uppercase tracking-wider text-zinc-500 dark:border-zinc-800 items-center px-2">
            <div></div><div></div><div></div>
            <div>Filename</div><div>Waveform</div><div class="text-right">Time</div><div class="text-center">Key</div><div class="text-center">BPM</div>
            <div></div><div></div>
        </div>
    </div>

    <div class="flex-1 overflow-y-auto px-8 pb-8" bind:this={scrollContainer}>
        {#if isLoading}
            <div class="flex justify-center items-center h-40 text-sm text-zinc-500 animate-pulse">Loading samples...</div>
        {:else}
            <div class="divide-y divide-zinc-100 dark:divide-zinc-800/50 mb-8">
                {#each samples as sample}
                    <div
                            class="group grid grid-cols-[20px_40px_32px_minmax(150px,2fr)_minmax(120px,1.5fr)_50px_40px_40px_32px_32px] items-center gap-4 py-2 transition-colors rounded-md -mx-2 px-2
                        {selectedId === sample.id ? 'bg-zinc-100 dark:bg-zinc-800/60' : 'hover:bg-zinc-50 dark:hover:bg-zinc-800/20'}"
                    >

                        <div class="flex justify-center"><input type="checkbox" class="h-4 w-4 rounded border-zinc-300 bg-zinc-100 cursor-pointer accent-zinc-900 dark:accent-zinc-100"></div>
                        <div class="h-10 w-10 flex items-center justify-center rounded-md bg-zinc-200/50 text-zinc-400 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700/50"><ImageIcon size={20} /></div>

                        <div class="flex justify-center">
                            <button
                                    onclick={() => handlePlayRequest(sample)}
                                    class="flex h-8 w-8 items-center justify-center rounded-full bg-zinc-900 text-zinc-100 hover:scale-105 dark:bg-zinc-100 dark:text-zinc-900 transition-transform cursor-pointer shadow-sm"
                            >
                                {#if playingId === sample.id} <Pause size={14} /> {:else} <Play size={14} class="ml-0.5" /> {/if}
                            </button>
                        </div>

                        <div class="flex flex-col min-w-0 pr-4" onclick={() => { selectedId = sample.id; }}>
                            <span class="truncate text-sm font-semibold cursor-pointer hover:underline" title={sample.original_path}>{sample.filename}</span>
                            <div class="flex gap-1.5 mt-1"><span class="rounded bg-zinc-200/60 px-1.5 py-0.5 text-[10px] font-medium text-zinc-700 dark:bg-zinc-800 dark:text-zinc-400">{sample.instrument_type || "Unknown"}</span></div>
                        </div>

                        <div class="flex items-center gap-[2px] h-8 overflow-hidden opacity-60 group-hover:opacity-100 transition-opacity">
                            {#each parseWaveform(sample.waveform_data) as barHeight, i}
                                <div
                                        class="w-[3px] rounded-full transition-colors
                                    {playingId === sample.id && (i / 40) <= playbackProgress ? 'bg-emerald-500' : 'bg-zinc-300 dark:bg-zinc-700'}"
                                        style="height: {barHeight}%;"
                                ></div>
                            {/each}
                        </div>

                        <div class="text-right text-xs font-medium text-zinc-500 tabular-nums">{formatDuration(sample.duration_ms)}</div>
                        <div class="text-center text-xs font-semibold text-zinc-700 dark:text-zinc-300">{sample.key_signature || "--"}</div>
                        <div class="text-center text-xs font-semibold text-zinc-700 dark:text-zinc-300">{sample.bpm ? Math.round(sample.bpm) : "--"}</div>
                        <div class="flex justify-center"><button class="text-zinc-400 hover:text-red-500 transition-colors opacity-0 group-hover:opacity-100 cursor-pointer"><Heart size={16} /></button></div>
                        <div class="flex justify-center"><button class="text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors opacity-0 group-hover:opacity-100 cursor-pointer"><EllipsisVertical size={16} /></button></div>

                    </div>
                {/each}
            </div>

            {#if totalPages > 1}
                <div class="flex items-center justify-center pb-8 pt-4">
                    <div class="flex items-center gap-1">
                        <button onclick={prevPage} disabled={currentPage === 1} class="flex items-center justify-center h-8 w-8 rounded text-zinc-600 hover:bg-zinc-100 disabled:opacity-30 disabled:hover:bg-transparent dark:text-zinc-400 dark:hover:bg-zinc-800 transition-colors cursor-pointer mr-2"><ChevronLeft size={18} /></button>
                        {#each visiblePages as pageNum}
                            <button onclick={() => goToPage(pageNum)} class="flex items-center justify-center h-8 w-8 rounded text-sm font-medium transition-colors cursor-pointer {pageNum === currentPage ? 'bg-zinc-900 text-white dark:bg-zinc-100 dark:text-zinc-900' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800'}">{pageNum}</button>
                        {/each}
                        <button onclick={nextPage} disabled={currentPage === totalPages} class="flex items-center justify-center h-8 w-8 rounded text-zinc-600 hover:bg-zinc-100 disabled:opacity-30 disabled:hover:bg-transparent dark:text-zinc-400 dark:hover:bg-zinc-800 transition-colors cursor-pointer ml-2"><ChevronRight size={18} /></button>
                    </div>
                </div>
            {/if}
        {/if}
    </div>
</div>