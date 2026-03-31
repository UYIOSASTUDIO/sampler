<script lang="ts">
    import { onMount, onDestroy, tick } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { open } from '@tauri-apps/plugin-dialog';
    import { EllipsisVertical, Download, Heart, Play, Pause, FolderPlus, RefreshCw, Trash2, Image as ImageIcon, ChevronLeft, ChevronRight, Settings, X } from 'lucide-svelte';
    import { appState } from '$lib/store.svelte';

    type SampleRecord = {
        id: string;
        filename: string;
        original_path: string;
        duration_ms: number;
        bpm: number | null;
        key_signature: string | null;
        instrument_type: string | null;
        waveform_data: number[] | null;
    };

    type PaginatedResponse = {
        samples: SampleRecord[];
        total_count: number;
    };

    let searchQuery: string = $state('');
    let searchTimeout: ReturnType<typeof setTimeout>;

    function handleSearchInput() {
        clearTimeout(searchTimeout);
        searchTimeout = setTimeout(() => {
            currentPage = 1;
            loadSamples();
        }, 300);
    }

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

    // --- ZERO-LATENCY WEB AUDIO API & RAM CACHE ---
    let audioCtx: AudioContext;
    let gainNode: GainNode; // NEU: Der globale Lautstärkeregler
    let sourceNode: AudioBufferSourceNode | null = null;
    let playingId: string | null = $state(null);
    let selectedId: string | null = $state(null);

    const audioRamCache = new Map<string, AudioBuffer>();

    let playbackProgress: number = $state(0);
    let currentSampleDuration: number = 0;
    let playbackStartTime: number = 0;
    let animationFrameId: number;

    const filters = ["Instruments", "Genres", "Key", "BPM", "One-Shots & Loops"];
    const instrumentTypes = ["Kick", "Snare", "Clap", "HiHat", "Cymbal", "Percussion", "Bass", "Vocal", "FX", "Synth", "Loop"];

    // --- PROGRESS BAR STATE ---
    type ScanProgressPayload = { total: number; current: number; current_file: string; };
    let scanTotal: number = $state(0);
    let scanCurrent: number = $state(0);
    let scanCurrentFile: string = $state('');
    let scanPercentage = $derived(scanTotal > 0 ? Math.round((scanCurrent / scanTotal) * 100) : 0);

    function updateProgress() {
        if (playingId && currentSampleDuration > 0 && audioCtx) {
            const elapsed = audioCtx.currentTime - playbackStartTime;

            const progress = Math.min(elapsed / currentSampleDuration, 1.0);

            // DER FIX: Lokalen UND globalen State gleichzeitig aktualisieren
            playbackProgress = progress;
            appState.playbackProgress = progress;

            if (progress >= 1.0) {
                cancelAnimationFrame(animationFrameId);
            } else {
                animationFrameId = requestAnimationFrame(updateProgress);
            }
        }
    }

    onMount(async () => {
        // @ts-ignore
        const AudioContextClass = window.AudioContext || window.webkitAudioContext;
        audioCtx = new AudioContextClass();

        // Master Volume Node initialisieren
        gainNode = audioCtx.createGain();
        gainNode.connect(audioCtx.destination);
        gainNode.gain.value = appState.globalVolume;

        window.addEventListener('keydown', handleKeydown);

        try {
            await invoke<number>('cleanup_database');
            await loadSamples();
        } catch (e) { console.error(e); }

        await listen('library-updated', () => loadSamples());
        await listen<ScanProgressPayload>('scan-progress', (e) => {
            scanTotal = e.payload.total; scanCurrent = e.payload.current; scanCurrentFile = e.payload.current_file;
        });
    });

    // Reagiere live auf den Fader im Footer
    $effect(() => {
        if (gainNode) gainNode.gain.value = appState.globalVolume;
    });

    // Reagiere auf die Buttons im Footer
    let lastToggle = $state(0); let lastNext = $state(0); let lastPrev = $state(0);
    $effect(() => {
        if (appState.cmdTogglePlay > lastToggle) {
            lastToggle = appState.cmdTogglePlay;
            if (appState.currentSample) {
                if (appState.isPlaying) {
                    if (sourceNode) { sourceNode.onended = null; sourceNode.stop(); }
                    appState.isPlaying = false; playingId = null; cancelAnimationFrame(animationFrameId);
                } else {
                    handlePlayRequest(appState.currentSample, true);
                }
            }
        }
        if (appState.cmdNext > lastNext) { lastNext = appState.cmdNext; playNextSample(); }
        if (appState.cmdPrev > lastPrev) { lastPrev = appState.cmdPrev; playPrevSample(); }
    });

    onDestroy(() => {
        if (sourceNode) sourceNode.stop();
        if (audioCtx) audioCtx.close();
        cancelAnimationFrame(animationFrameId);
        window.removeEventListener('keydown', handleKeydown);
    });

    async function loadSamples() {
        isLoading = true;
        try {
            const response = await invoke<PaginatedResponse>('get_samples', { filterType: activeTypeFilter, searchQuery: searchQuery.trim() !== '' ? searchQuery.trim() : null, page: currentPage, pageSize: pageSize });
            samples = response.samples; totalItems = response.total_count;
            if (scrollContainer) scrollContainer.scrollTop = 0;
            if (sourceNode) { sourceNode.stop(); playingId = null; }
        } catch (error) { console.error(error); } finally { isLoading = false; }
    }

    // Ausgelagerte Navigations-Logik für Tasten UND Footer-Buttons
    async function playNextSample() {
        if (samples.length === 0) return;
        let currentIndex = samples.findIndex(s => s.id === selectedId);

        if (currentIndex === -1) {
            // Nichts ausgewählt -> Starte beim ersten Sound der aktuellen Seite
            currentIndex = 0;
        } else if (currentIndex === samples.length - 1) {
            // Am Ende angekommen -> Nächste Seite
            if (currentPage < totalPages) {
                currentPage++; await loadSamples();
                if (samples.length > 0) {
                    await handlePlayRequest(samples[0]);
                    setTimeout(() => document.getElementById(`sample-${samples[0].id}`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' }), 50);
                }
            }
            return;
        } else {
            currentIndex++;
        }

        await handlePlayRequest(samples[currentIndex]);
        setTimeout(() => document.getElementById(`sample-${samples[currentIndex].id}`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' }), 50);
    }

    async function playPrevSample() {
        if (samples.length === 0) return;
        let currentIndex = samples.findIndex(s => s.id === selectedId);

        if (currentIndex === -1) {
            // Nichts ausgewählt -> Starte beim letzten Sound der aktuellen Seite
            currentIndex = samples.length - 1;
        } else if (currentIndex === 0) {
            // Am Anfang angekommen -> Vorherige Seite
            if (currentPage > 1) {
                currentPage--; await loadSamples();
                if (samples.length > 0) {
                    await handlePlayRequest(samples[samples.length - 1]);
                    setTimeout(() => document.getElementById(`sample-${samples[samples.length - 1].id}`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' }), 50);
                }
            }
            return;
        } else {
            currentIndex--;
        }

        await handlePlayRequest(samples[currentIndex]);
        setTimeout(() => document.getElementById(`sample-${samples[currentIndex].id}`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' }), 50);
    }

    // --- ENTERPRISE KEYBOARD NAVIGATION ---
    async function handleKeydown(e: KeyboardEvent) {
        if (document.activeElement?.tagName === 'INPUT') return;

        if (isLoading) {
            if (['ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', ' '].includes(e.key)) {
                e.preventDefault();
            }
            return;
        }

        if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault();
            if (samples.length === 0) return;

            let currentIndex = samples.findIndex(s => s.id === selectedId);

            if (e.key === 'ArrowDown') {
                if (currentIndex === -1) {
                    // Start at the very first sample
                    currentIndex = 0;
                } else if (currentIndex === samples.length - 1) {
                    // Next page
                    if (currentPage < totalPages) {
                        currentPage++;
                        await loadSamples();
                        if (samples.length > 0) {
                            const nextSample = samples[0];
                            await handlePlayRequest(nextSample);
                            await tick();
                            document.getElementById(`sample-${nextSample.id}`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
                        }
                    }
                    return;
                } else {
                    currentIndex++;
                }
            } else if (e.key === 'ArrowUp') {
                if (currentIndex === -1) {
                    // Start at the very last sample (oder 0, aber unten macht mehr Sinn beim Hochscrollen)
                    currentIndex = samples.length - 1;
                } else if (currentIndex === 0) {
                    // Previous page
                    if (currentPage > 1) {
                        currentPage--;
                        await loadSamples();
                        if (samples.length > 0) {
                            const nextSample = samples[samples.length - 1];
                            await handlePlayRequest(nextSample);
                            await tick();
                            document.getElementById(`sample-${nextSample.id}`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
                        }
                    }
                    return;
                } else {
                    currentIndex--;
                }
            }

            const nextSample = samples[currentIndex];
            handlePlayRequest(nextSample);

            await tick();
            const element = document.getElementById(`sample-${nextSample.id}`);
            if (element) {
                element.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
            }

        } else if (e.key === 'ArrowLeft') {
            e.preventDefault();
            if (!selectedId) return;

            const currentSample = samples.find(s => s.id === selectedId);
            if (!currentSample) return;

            // Retrigger (Neustart)
            handlePlayRequest(currentSample, true);

        } else if (e.key === ' ') {
            // Leertaste für Play/Pause
            e.preventDefault();
            if (!selectedId) return;

            const currentSample = samples.find(s => s.id === selectedId);
            if (!currentSample) return;

            handlePlayRequest(currentSample);
        }
    }

    async function handlePlayRequest(sample: SampleRecord, forceRestart: boolean = false) {
        selectedId = sample.id;

        if (playingId === sample.id && !forceRestart) {
            if (sourceNode) { sourceNode.onended = null; sourceNode.stop(); }
            playingId = null; appState.isPlaying = false; appState.playbackProgress = 0; cancelAnimationFrame(animationFrameId);
            return;
        }

        if (sourceNode) { sourceNode.onended = null; try { sourceNode.stop(); } catch(e) {} }
        cancelAnimationFrame(animationFrameId);

        playingId = sample.id;
        appState.playbackProgress = 0; // Harter Reset für das neue Sample

        let audioBuffer = audioRamCache.get(sample.id);
        if (!audioBuffer) {
            try {
                const assetUrl = convertFileSrc(sample.original_path);
                const response = await fetch(assetUrl);
                audioBuffer = await audioCtx.decodeAudioData(await response.arrayBuffer());
                if (audioRamCache.size >= 200) { const firstKey = audioRamCache.keys().next().value; if (firstKey) audioRamCache.delete(firstKey); }
                audioRamCache.set(sample.id, audioBuffer);
            } catch (error) { console.error(error); playingId = null; return; }
        }

        if (playingId !== sample.id) return;

        appState.currentSample = sample;
        appState.isPlaying = true;

        sourceNode = audioCtx.createBufferSource();
        sourceNode.buffer = audioBuffer;
        sourceNode.connect(gainNode);

        sourceNode.onended = () => {
            if (playingId === sample.id) {
                // DER FIX: 1. Zwinge die Anzeige auf exakt 100% am rechten Rand
                appState.playbackProgress = 1.0;
                playingId = null;
                appState.isPlaying = false;
                cancelAnimationFrame(animationFrameId);

                // DER FIX: 2. Warte 150ms, damit der User das Ende sieht, bevor der Balken auf 0 springt
                setTimeout(() => {
                    // Nur auf 0 setzen, wenn der User in diesen 150ms nicht schon ein anderes Sample gestartet hat
                    if (!appState.isPlaying) {
                        appState.playbackProgress = 0;
                    }
                }, 150);
            }
        };

        currentSampleDuration = audioBuffer.duration;
        playbackStartTime = audioCtx.currentTime;
        sourceNode.start(0);
        animationFrameId = requestAnimationFrame(updateProgress);
    }

    function goToPage(p: number) { if (p !== currentPage) { currentPage = p; loadSamples(); } }
    function nextPage() { if (currentPage < totalPages) { currentPage++; loadSamples(); } }
    function prevPage() { if (currentPage > 1) { currentPage--; loadSamples(); } }
    function toggleFilter(type: string) { activeTypeFilter = activeTypeFilter === type ? null : type; currentPage = 1; loadSamples(); }

    function parseWaveform(data: number[] | null): number[] {
        if (!data || data.length === 0) return Array(40).fill(10);
        return data;
    }

    async function handleSelectFolder() { try { const result = await open({ directory: true, multiple: false }); if (result) { selectedPath = result as string; scanMessage = 'Ready to scan.'; } } catch (error) { console.error(error); } }
    async function handleScan() { if (!selectedPath) return; isScanning = true; scanMessage = 'Indexing...'; try { const count = await invoke<number>('scan_library', { path: selectedPath }); scanMessage = `Added ${count} files.`; selectedPath = null; currentPage = 1; await loadSamples(); } catch (error) { scanMessage = `Error: ${error}`; } finally { isScanning = false; } }
    async function handleClearDatabase() { if (confirm("Clear the entire library?")) { isScanning = true; try { await invoke('clear_database'); activeTypeFilter = null; currentPage = 1; samples = []; totalItems = 0; scanMessage = 'Library cleared.'; } catch (error) { scanMessage = `Error: ${error}`; } finally { isScanning = false; } } }

    function formatDuration(ms: number): string {
        if (ms === 0) return "--:--";
        const totalSec = Math.floor(ms / 1000);
        return `${Math.floor(totalSec / 60)}:${(totalSec % 60).toString().padStart(2, '0')}`;
    }

    async function handleRescanAll() {
        isScanning = true;
        scanMessage = 'Syncing folders...';
        try {
            const removed = await invoke<number>('cleanup_database');
            const added = await invoke<number>('rescan_all_folders');

            scanMessage = `Synced! Added: ${added}, Removed: ${removed}`;
            currentPage = 1;
            await loadSamples();

            setTimeout(() => { scanMessage = ''; }, 3000);
        } catch (error) {
            scanMessage = `Error: ${error}`;
        } finally {
            isScanning = false;
        }
    }

    // --- SETTINGS MODAL LOGIC ---
    let activeSettingsTab: 'general' | 'library' | 'audio' = $state('general');
    let connectedFolders: string[] = $state([]);
    let isSettingsLoading: boolean = $state(false);

    function setThemePref(pref: 'light' | 'dark' | 'system') {
        appState.themePreference = pref;
        localStorage.setItem('samplevault-theme', pref);
    }

    $effect(() => {
        if (appState.isSettingsOpen) {
            loadConnectedFolders();
        }
    });

    async function loadConnectedFolders() {
        isSettingsLoading = true;
        try {
            connectedFolders = await invoke<string[]>('get_connected_folders');
        } catch (error) {
            console.error("Failed to load connected folders:", error);
        } finally {
            isSettingsLoading = false;
        }
    }

    async function handleRemoveFolder(folderPath: string) {
        if (!confirm(`Un-link this folder?\n\n${folderPath}\n\nThis will remove all its samples from your library.`)) return;

        isSettingsLoading = true;
        try {
            await invoke('remove_folder', { path: folderPath });
            await loadConnectedFolders();
            currentPage = 1;
            await loadSamples();
        } catch (error) {
            console.error("Failed to remove folder:", error);
        } finally {
            isSettingsLoading = false;
        }
    }
</script>

{#if appState.currentView === 'sounds'}

    <div class="h-full w-full overflow-y-auto" bind:this={scrollContainer}>

        <div class="px-8 pt-8 pb-2">
            <div class="mb-6 flex items-end justify-between border-b border-zinc-200 pb-0 dark:border-zinc-800">

                <div class="flex-1">
                    <div class="flex items-center gap-6 mb-4">
                        <h1 class="text-3xl font-bold tracking-tight">Sounds</h1>

                        <div class="relative max-w-md w-full ml-4">
                            <input
                                    type="text"
                                    bind:value={searchQuery}
                                    oninput={handleSearchInput}
                                    placeholder="Search in 1,000,000+ samples..."
                                    class="w-full rounded-md border border-zinc-300 bg-white px-4 py-2 text-sm text-zinc-900 focus:border-zinc-900 focus:outline-none focus:ring-1 focus:ring-zinc-900 dark:border-zinc-700 dark:bg-[#18181b] dark:text-zinc-100 dark:focus:border-zinc-100 dark:focus:ring-zinc-100"
                            >
                        </div>
                    </div>

                    <div class="flex gap-6">
                        <button class="border-b-2 border-zinc-900 pb-2 text-sm font-semibold text-zinc-900 dark:border-zinc-100 dark:text-zinc-100">Samples</button>
                        <button class="pb-2 text-sm font-medium text-zinc-500 hover:text-zinc-900 dark:text-zinc-400">Presets</button>
                        <button class="pb-2 text-sm font-medium text-zinc-500 hover:text-zinc-900 dark:text-zinc-400">Packs</button>
                    </div>
                </div>

                <div class="flex flex-col items-end gap-3 pb-2">
                    {#if isScanning && scanTotal > 0}
                        <div class="w-full flex flex-col gap-1 mt-1">
                            <div class="flex justify-between text-[10px] font-medium text-zinc-500 uppercase tracking-wider">
                                <span>Scanning: {scanCurrent} / {scanTotal}</span>
                                <span>{scanPercentage}%</span>
                            </div>
                            <div class="h-1.5 w-full rounded-full bg-zinc-200 overflow-hidden dark:bg-zinc-800">
                                <div
                                        class="h-full bg-blue-500 transition-all duration-300 ease-out"
                                        style="width: {scanPercentage}%"
                                ></div>
                            </div>
                            <div class="text-[10px] text-zinc-400 truncate text-right">
                                {scanCurrentFile}
                            </div>
                        </div>
                    {:else if scanMessage}
                        <span class="text-xs font-medium text-zinc-500 animate-pulse">{scanMessage}</span>
                    {/if}

                    <div class="flex items-center gap-2">

                        <button
                                onclick={handleRescanAll}
                                disabled={isScanning}
                                class="flex items-center gap-1.5 rounded-md border border-blue-700/50 bg-blue-50 px-3 py-1.5 text-xs font-medium text-blue-700 hover:bg-blue-100 dark:border-blue-500/30 dark:bg-blue-500/10 dark:text-blue-400 cursor-pointer disabled:opacity-50"
                        >
                            <RefreshCw size={14} class={isScanning ? "animate-spin" : ""} /> Sync
                        </button>

                        <button onclick={handleSelectFolder} disabled={isScanning} class="flex items-center gap-1.5 rounded-md border border-zinc-200 bg-white px-3 py-1.5 text-xs font-medium hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-800 dark:hover:bg-zinc-700 transition-colors disabled:opacity-50 cursor-pointer">
                            <FolderPlus size={14} /> Add Folder
                        </button>

                        {#if selectedPath}
                            <button onclick={handleScan} disabled={isScanning} class="flex items-center gap-1.5 rounded-md border border-emerald-700/50 bg-emerald-50 px-3 py-1.5 text-xs font-medium text-emerald-700 hover:bg-emerald-100 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-400 cursor-pointer">
                                <Play size={14} /> Scan New
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
                        <button class="flex items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 py-1.5 text-xs font-medium hover:bg-zinc-50 dark:border-zinc-700/50 dark:bg-zinc-800/30 dark:hover:bg-zinc-800 transition-colors cursor-pointer">
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

            <div class="mb-4 flex items-center justify-between text-sm text-zinc-500">
                <span>{#if isLoading}Loading...{:else}{totalItems} results{/if}</span>
                <div class="flex items-center gap-2">
                    <span class="text-xs font-medium">Sort by:</span>
                    <button class="text-xs font-semibold text-zinc-900 dark:text-zinc-100 cursor-pointer">Most recent ▼</button>
                </div>
            </div>

            <div class="grid grid-cols-[20px_40px_32px_minmax(150px,2fr)_minmax(120px,1.5fr)_50px_40px_40px_32px_32px] gap-4 border-b border-zinc-200 pb-2 text-[11px] font-semibold uppercase tracking-wider text-zinc-500 dark:border-zinc-800 items-center px-2">
                <div></div><div></div><div></div>
                <div>Filename</div><div>Waveform</div><div class="text-right">Time</div><div class="text-center">Key</div><div class="text-center">BPM</div>
                <div></div><div></div>
            </div>
        </div>

        <div class="px-8 pb-8">
            {#if isLoading}
                <div class="flex justify-center items-center h-40 text-sm text-zinc-500 animate-pulse">Loading samples...</div>
            {:else}
                <div class="divide-y divide-zinc-100 dark:divide-zinc-800/50 mb-8">
                    {#each samples as sample}
                        <div
                                id="sample-{sample.id}"
                                class="group grid grid-cols-[20px_40px_32px_minmax(150px,2fr)_minmax(120px,1.5fr)_50px_40px_40px_32px_32px] items-center gap-4 py-2 rounded-md -mx-2 px-2
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

                            <div
                                    class="flex flex-col min-w-0 pr-4 cursor-pointer"
                                    role="button"
                                    tabindex="0"
                                    onclick={() => { selectedId = sample.id; }}
                                    onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') selectedId = sample.id; }}
                            >
                                <span class="truncate text-sm font-semibold cursor-pointer hover:underline" title={sample.original_path}>{sample.filename}</span>
                                <div class="flex gap-1.5 mt-1"><span class="rounded bg-zinc-200/60 px-1.5 py-0.5 text-[10px] font-medium text-zinc-700 dark:bg-zinc-800 dark:text-zinc-400">{sample.instrument_type || "Unknown"}</span></div>
                            </div>

                            <div class="flex items-center gap-[2px] h-8 overflow-hidden opacity-60 group-hover:opacity-100 transition-opacity">
                                {#each parseWaveform(sample.waveform_data) as barHeight, i}
                                    <div
                                            class="w-[3px] rounded-full
                                        {playingId === sample.id && (i / 40) <= playbackProgress ? 'bg-emerald-500' : 'bg-zinc-300 dark:bg-zinc-700'}"
                                            style="height: {barHeight}%;"
                                    ></div>
                                {/each}
                            </div>

                            <div class="text-right text-xs font-medium text-zinc-500 tabular-nums">{formatDuration(sample.duration_ms)}</div>
                            <div class="text-center text-xs font-semibold text-zinc-700 dark:text-zinc-300">{sample.key_signature || "--"}</div>
                            <div class="text-center text-xs font-semibold text-zinc-700 dark:text-zinc-300">{sample.bpm ? Math.round(sample.bpm) : "--"}</div>
                            <div class="flex justify-center">
                                <button
                                        class="text-zinc-400 hover:text-red-500 transition-colors cursor-pointer group-hover:opacity-100 {selectedId === sample.id ? 'opacity-100' : 'opacity-0'}"
                                >
                                    <Heart size={16} />
                                </button>
                            </div>
                            <div class="flex justify-center">
                                <button
                                        class="text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer group-hover:opacity-100 {selectedId === sample.id ? 'opacity-100' : 'opacity-0'}"
                                >
                                    <EllipsisVertical size={16} />
                                </button>
                            </div>

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

{:else if appState.currentView === 'projects'}
    <div class="flex h-full items-center justify-center text-zinc-500">
        <h2 class="text-2xl font-bold">Musik Projekte (Coming Soon)</h2>
    </div>

{:else if appState.currentView === 'editor'}
    <div class="flex h-full items-center justify-center text-zinc-500">
        <h2 class="text-2xl font-bold">Pack Editor & Renamer (Coming Soon)</h2>
    </div>

{/if}


{#if appState.isSettingsOpen}
    <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm">
        <div class="w-full max-w-2xl overflow-hidden rounded-xl border border-zinc-200 bg-white shadow-2xl dark:border-zinc-800 dark:bg-[#18181b]">

            <div class="flex items-center justify-between border-b border-zinc-100 px-6 py-4 dark:border-zinc-800/50">
                <h2 class="text-lg font-bold text-zinc-900 dark:text-zinc-100">Preferences</h2>
                <button onclick={() => appState.isSettingsOpen = false} class="text-zinc-400 hover:text-zinc-900 transition-colors dark:hover:text-zinc-100 cursor-pointer">
                    <X size={20} />
                </button>
            </div>

            <div class="flex h-[400px]">

                <div class="w-1/4 border-r border-zinc-100 bg-zinc-50/50 p-4 space-y-1 dark:border-zinc-800/50 dark:bg-black/20">
                    <button onclick={() => activeSettingsTab = 'general'} class="w-full rounded-md px-3 py-2 text-left text-sm font-semibold transition-colors cursor-pointer {activeSettingsTab === 'general' ? 'bg-zinc-200/50 text-zinc-900 dark:bg-zinc-800 dark:text-zinc-100' : 'text-zinc-500 hover:bg-zinc-200/50 hover:text-zinc-900 dark:hover:bg-zinc-800/50 dark:hover:text-zinc-100'}">General</button>
                    <button onclick={() => activeSettingsTab = 'library'} class="w-full rounded-md px-3 py-2 text-left text-sm font-semibold transition-colors cursor-pointer {activeSettingsTab === 'library' ? 'bg-zinc-200/50 text-zinc-900 dark:bg-zinc-800 dark:text-zinc-100' : 'text-zinc-500 hover:bg-zinc-200/50 hover:text-zinc-900 dark:hover:bg-zinc-800/50 dark:hover:text-zinc-100'}">Library</button>
                    <button onclick={() => activeSettingsTab = 'audio'} class="w-full rounded-md px-3 py-2 text-left text-sm font-semibold transition-colors cursor-pointer {activeSettingsTab === 'audio' ? 'bg-zinc-200/50 text-zinc-900 dark:bg-zinc-800 dark:text-zinc-100' : 'text-zinc-500 hover:bg-zinc-200/50 hover:text-zinc-900 dark:hover:bg-zinc-800/50 dark:hover:text-zinc-100'}">Audio</button>
                </div>

                <div class="flex-1 overflow-y-auto p-6">

                    {#if activeSettingsTab === 'general'}
                        <h3 class="mb-6 text-xs font-bold uppercase tracking-wider text-zinc-400">Appearance</h3>

                        <div class="space-y-4">
                            <label class="text-sm font-medium text-zinc-900 dark:text-zinc-100">Theme Preference</label>
                            <div class="flex rounded-lg bg-zinc-100 p-1 dark:bg-zinc-800/50 border border-zinc-200 dark:border-zinc-700/50 w-fit">
                                <button
                                        onclick={() => setThemePref('light')}
                                        class="px-4 py-1.5 text-xs font-semibold rounded-md transition-all cursor-pointer {appState.themePreference === 'light' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}"
                                >Light</button>
                                <button
                                        onclick={() => setThemePref('dark')}
                                        class="px-4 py-1.5 text-xs font-semibold rounded-md transition-all cursor-pointer {appState.themePreference === 'dark' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}"
                                >Dark</button>
                                <button
                                        onclick={() => setThemePref('system')}
                                        class="px-4 py-1.5 text-xs font-semibold rounded-md transition-all cursor-pointer {appState.themePreference === 'system' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}"
                                >System</button>
                            </div>
                            <p class="text-xs text-zinc-500">
                                "System" automatically matches your Mac's appearance settings.
                            </p>
                        </div>

                    {:else if activeSettingsTab === 'library'}
                        <h3 class="mb-4 text-xs font-bold uppercase tracking-wider text-zinc-400">Connected Folders</h3>

                        {#if isSettingsLoading}
                            <div class="flex h-20 items-center justify-center text-sm text-zinc-500 animate-pulse">Loading library data...</div>
                        {:else if connectedFolders.length === 0}
                            <div class="flex h-20 items-center justify-center rounded-md border border-dashed border-zinc-200 text-sm text-zinc-500 dark:border-zinc-800">
                                No folders connected yet.
                            </div>
                        {:else}
                            <div class="space-y-2">
                                {#each connectedFolders as folder}
                                    <div class="flex items-center justify-between rounded-md border border-zinc-200 bg-white p-3 shadow-sm dark:border-zinc-800 dark:bg-zinc-900/50">
                                        <div class="flex flex-col overflow-hidden pr-4">
                                            <span class="truncate text-sm font-medium text-zinc-700 dark:text-zinc-300" title={folder}>{folder}</span>
                                        </div>
                                        <button onclick={() => handleRemoveFolder(folder)} disabled={isSettingsLoading} class="shrink-0 rounded-md border border-red-200 bg-red-50 px-3 py-1.5 text-xs font-medium text-red-700 hover:bg-red-100 transition-colors dark:border-red-900/30 dark:bg-red-900/10 dark:text-red-400 dark:hover:bg-red-900/20 cursor-pointer disabled:opacity-50">
                                            Un-link
                                        </button>
                                    </div>
                                {/each}
                            </div>
                        {/if}
                        <p class="mt-4 text-xs text-zinc-500">
                            Un-linking a folder removes all its indexed samples from this application. It does not delete the actual files from your computer.
                        </p>

                    {:else if activeSettingsTab === 'audio'}
                        <h3 class="mb-4 text-xs font-bold uppercase tracking-wider text-zinc-400">Audio Engine</h3>
                        <div class="flex h-20 items-center justify-center rounded-md border border-dashed border-zinc-200 text-sm text-zinc-500 dark:border-zinc-800">
                            Audio Device Options (Coming Soon)
                        </div>
                    {/if}
                </div>

            </div>
        </div>
    </div>
{/if}