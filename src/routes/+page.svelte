<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { open } from '@tauri-apps/plugin-dialog';
    import { EllipsisVertical, Download, Heart, Play, Pause, FolderPlus, RefreshCw, Trash2, Image as ImageIcon } from 'lucide-svelte';

    type SampleRecord = {
        id: string;
        filename: string;
        original_path: string;
        duration_ms: number;
        bpm: number | null;
        key_signature: string | null;
        instrument_type: string | null;
    };

    let samples: SampleRecord[] = $state([]);
    let isLoading: boolean = $state(true);

    let selectedPath: string | null = $state(null);
    let isScanning: boolean = $state(false);
    let scanMessage: string = $state('');
    let activeTypeFilter: string | null = $state(null);

    // --- NEU: Audio State Management ---
    let currentAudio: HTMLAudioElement | null = null;
    let playingId: string | null = $state(null);
    let currentBlobUrl: string | null = null;

    const filters = ["Instruments", "Genres", "Key", "BPM", "One-Shots & Loops"];
    const instrumentTypes = [
        "Kick", "Snare", "Clap", "HiHat", "Cymbal",
        "Percussion", "Bass", "Vocal", "FX", "Synth", "Loop"
    ];

    onMount(async () => {
        await loadSamples();
    });

    // Speicherlecks beim Verlassen der Komponente verhindern
    onDestroy(() => {
        if (currentAudio) {
            currentAudio.pause();
        }
        if (currentBlobUrl) {
            URL.revokeObjectURL(currentBlobUrl);
        }
    });

    async function loadSamples() {
        isLoading = true;
        try {
            samples = await invoke<SampleRecord[]>('get_samples', { filterType: activeTypeFilter });
        } catch (error) {
            console.error("Critical: Failed to load samples from database", error);
        } finally {
            isLoading = false;
        }
    }

    function toggleFilter(type: string) {
        if (activeTypeFilter === type) {
            activeTypeFilter = null;
        } else {
            activeTypeFilter = type;
        }
        loadSamples();
    }

    // --- NEU: Audio Playback Engine ---
    async function togglePlay(sample: SampleRecord) {
        // Wenn derselbe Sound läuft, pausiere ihn
        if (playingId === sample.id && currentAudio) {
            currentAudio.pause();
            playingId = null;
            return;
        }

        // Wenn ein anderer Sound läuft, stoppe diesen und räume den RAM auf
        if (currentAudio) {
            currentAudio.pause();
            if (currentBlobUrl) URL.revokeObjectURL(currentBlobUrl);
        }

        try {
            playingId = sample.id; // UI reagiert sofort

            // Lade die Audio-Bytes aus dem Rust-Backend
            const bytes = await invoke<number[]>('read_audio_file', { path: sample.original_path });
            const uint8Array = new Uint8Array(bytes);

            // Konvertiere Bytes zu Blob und generiere eine interne URL
            const blob = new Blob([uint8Array]);
            currentBlobUrl = URL.createObjectURL(blob);

            currentAudio = new Audio(currentBlobUrl);
            currentAudio.onended = () => {
                playingId = null; // UI Play-Button zurücksetzen, wenn fertig
            };
            await currentAudio.play();
        } catch (error) {
            console.error("Failed to play audio:", error);
            playingId = null;
        }
    }

    async function handleSelectFolder() {
        try {
            const result = await open({ directory: true, multiple: false });
            if (result) {
                selectedPath = result as string;
                scanMessage = 'Folder selected. Ready to scan.';
            }
        } catch (error) {
            console.error("Failed to open dialog:", error);
        }
    }

    async function handleScan() {
        if (!selectedPath) return;
        isScanning = true;
        scanMessage = 'Indexing...';
        try {
            const processedCount = await invoke<number>('scan_library', { path: selectedPath });
            scanMessage = `Added ${processedCount} new files.`;
            selectedPath = null;
            await loadSamples();
        } catch (error) {
            scanMessage = `Error: ${error}`;
        } finally {
            isScanning = false;
        }
    }

    async function handleClearDatabase() {
        if (confirm("Are you sure you want to clear the entire library?")) {
            isScanning = true;
            try {
                await invoke('clear_database');
                activeTypeFilter = null;
                samples = [];
                scanMessage = 'Library cleared.';
            } catch (error) {
                scanMessage = `Error: ${error}`;
            } finally {
                isScanning = false;
            }
        }
    }

    function formatDuration(ms: number): string {
        if (ms === 0) return "--:--";
        const totalSeconds = Math.floor(ms / 1000);
        const minutes = Math.floor(totalSeconds / 60);
        const seconds = totalSeconds % 60;
        return `${minutes}:${seconds.toString().padStart(2, '0')}`;
    }

    // Hilfsfunktion: Generiert ein Array von zufälligen Höhen für die Waveform-Mockups
    function generateWaveformBars(id: string) {
        // Nutzen die ID als Seed, damit die Wellenform pro Sample immer gleich aussieht
        const seed = parseInt(id.substring(0, 8), 16) || 123;
        const bars = [];
        for (let i = 0; i < 40; i++) {
            const height = 10 + (Math.sin(seed + i) * Math.cos(seed * i) * 10 + 10);
            bars.push(Math.abs(height));
        }
        return bars;
    }
</script>

<div class="flex flex-col h-full p-8">
    <div class="mb-6 flex items-end justify-between border-b border-zinc-200 pb-0 dark:border-zinc-800">
        <div>
            <h1 class="text-3xl font-bold tracking-tight mb-4">Sounds</h1>
            <div class="flex gap-6">
                <button class="border-b-2 border-zinc-900 pb-2 text-sm font-semibold text-zinc-900 dark:border-zinc-100 dark:text-zinc-100">Samples</button>
                <button class="pb-2 text-sm font-medium text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100">Presets</button>
                <button class="pb-2 text-sm font-medium text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100">Packs</button>
            </div>
        </div>

        <div class="flex flex-col items-end gap-3 pb-2">
            {#if scanMessage}
                <span class="text-xs font-medium text-zinc-500 animate-pulse">{scanMessage}</span>
            {/if}
            <div class="flex items-center gap-2">
                <button
                        onclick={handleSelectFolder}
                        disabled={isScanning}
                        class="flex items-center gap-1.5 rounded-md border border-zinc-200 bg-white px-3 py-1.5 text-xs font-medium text-zinc-700 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700 transition-colors disabled:opacity-50 cursor-pointer"
                >
                    <FolderPlus size={14} />
                    Browse
                </button>

                {#if selectedPath}
                    <button
                            onclick={handleScan}
                            disabled={isScanning}
                            class="flex items-center gap-1.5 rounded-md border border-emerald-700/50 bg-emerald-50 px-3 py-1.5 text-xs font-medium text-emerald-700 hover:bg-emerald-100 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-400 dark:hover:bg-emerald-500/20 transition-colors cursor-pointer"
                    >
                        <RefreshCw size={14} class={isScanning ? "animate-spin" : ""} />
                        Start Scan
                    </button>
                {/if}

                <button
                        onclick={handleClearDatabase}
                        disabled={isScanning}
                        class="flex items-center gap-1.5 rounded-md border border-red-200 bg-red-50 px-3 py-1.5 text-xs font-medium text-red-700 hover:bg-red-100 dark:border-red-900/50 dark:bg-red-950/30 dark:text-red-400 dark:hover:bg-red-900/50 transition-colors disabled:opacity-50 cursor-pointer ml-2"
                >
                    <Trash2 size={14} />
                    Clear
                </button>
            </div>
        </div>
    </div>

    <div class="mb-6 space-y-4">
        <div class="flex flex-wrap gap-2">
            {#each filters as filter}
                <button class="flex items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 py-1.5 text-xs font-medium text-zinc-700 hover:bg-zinc-50 dark:border-zinc-700/50 dark:bg-zinc-800/30 dark:text-zinc-300 dark:hover:bg-zinc-800 transition-colors">
                    {filter}
                    <span class="text-[10px] opacity-50">▼</span>
                </button>
            {/each}
        </div>

        <div class="flex flex-wrap gap-2">
            {#each instrumentTypes as type}
                <button
                        onclick={() => toggleFilter(type)}
                        class="rounded-full border px-3 py-1 text-[11px] font-medium transition-colors cursor-pointer
                        {activeTypeFilter === type
                            ? 'bg-zinc-900 border-zinc-900 text-white dark:bg-zinc-100 dark:border-zinc-100 dark:text-zinc-900'
                            : 'bg-zinc-50 border-zinc-200 text-zinc-600 hover:border-zinc-300 dark:bg-[#18181b] dark:border-zinc-800 dark:text-zinc-400 dark:hover:border-zinc-600'}"
                >
                    {type}
                </button>
            {/each}
        </div>
    </div>

    <div class="mb-4 flex items-center justify-between text-sm text-zinc-500">
        <span>{#if isLoading}Loading...{:else}{samples.length} results{/if}</span>
        <div class="flex items-center gap-2">
            <span class="text-xs font-medium">Sort by:</span>
            <button class="text-xs font-semibold text-zinc-900 dark:text-zinc-100">Most recent ▼</button>
        </div>
    </div>

    <div class="flex-1">
        <div class="grid grid-cols-[20px_40px_32px_minmax(150px,2fr)_minmax(120px,1.5fr)_50px_40px_40px_32px_32px] gap-4 border-b border-zinc-200 pb-2 text-[11px] font-semibold uppercase tracking-wider text-zinc-500 dark:border-zinc-800 items-center px-2">
            <div></div> <div></div> <div></div> <div>Filename</div>
            <div>Waveform</div>
            <div class="text-right">Time</div>
            <div class="text-center">Key</div>
            <div class="text-center">BPM</div>
            <div></div> <div></div> </div>

        <div class="divide-y divide-zinc-100 dark:divide-zinc-800/50">
            {#each samples as sample}
                <div class="group grid grid-cols-[20px_40px_32px_minmax(150px,2fr)_minmax(120px,1.5fr)_50px_40px_40px_32px_32px] items-center gap-4 py-2 hover:bg-zinc-50 dark:hover:bg-zinc-800/20 transition-colors rounded-md -mx-2 px-2">

                    <div class="flex justify-center">
                        <input type="checkbox" class="h-4 w-4 rounded border-zinc-300 bg-zinc-100 text-zinc-900 focus:ring-zinc-900 dark:border-zinc-700 dark:bg-zinc-900 dark:checked:bg-zinc-100 dark:checked:border-zinc-100 cursor-pointer accent-zinc-900 dark:accent-zinc-100">
                    </div>

                    <div class="h-10 w-10 flex items-center justify-center rounded-md bg-zinc-100 text-zinc-400 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700/50 overflow-hidden">
                        <ImageIcon size={20} />
                    </div>

                    <div class="flex justify-center">
                        <button
                                onclick={() => togglePlay(sample)}
                                class="flex h-8 w-8 items-center justify-center rounded-full bg-zinc-900 text-zinc-100 hover:scale-105 dark:bg-zinc-100 dark:text-zinc-900 transition-transform cursor-pointer shadow-sm"
                        >
                            {#if playingId === sample.id}
                                <Pause size={14} />
                            {:else}
                                <Play size={14} class="ml-0.5" />
                            {/if}
                        </button>
                    </div>

                    <div class="flex flex-col min-w-0 pr-4">
                        <span class="truncate text-sm font-semibold text-zinc-900 dark:text-zinc-100 cursor-pointer hover:underline" title={sample.original_path}>
                            {sample.filename}
                        </span>
                        <div class="flex gap-1.5 mt-1">
                            <span class="rounded bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400">
                                {sample.instrument_type || "Unknown"}
                            </span>
                        </div>
                    </div>

                    <div class="flex items-center gap-[2px] h-8 overflow-hidden opacity-60 group-hover:opacity-100 transition-opacity">
                        {#each generateWaveformBars(sample.id) as barHeight}
                            <div
                                    class="w-[3px] rounded-full transition-colors duration-200 {playingId === sample.id ? 'bg-emerald-500' : 'bg-zinc-300 dark:bg-zinc-700'}"
                                    style="height: {barHeight}px;"
                            ></div>
                        {/each}
                    </div>

                    <div class="text-right text-xs font-medium text-zinc-500 tabular-nums">
                        {formatDuration(sample.duration_ms)}
                    </div>

                    <div class="text-center text-xs font-semibold text-zinc-700 dark:text-zinc-300">
                        {sample.key_signature || "--"}
                    </div>

                    <div class="text-center text-xs font-semibold text-zinc-700 dark:text-zinc-300">
                        {sample.bpm ? Math.round(sample.bpm) : "--"}
                    </div>

                    <div class="flex justify-center">
                        <button class="text-zinc-400 hover:text-red-500 transition-colors cursor-pointer opacity-0 group-hover:opacity-100"><Heart size={16} /></button>
                    </div>

                    <div class="flex justify-center">
                        <button class="text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer opacity-0 group-hover:opacity-100"><EllipsisVertical size={16} /></button>
                    </div>
                </div>
            {/each}
        </div>
    </div>
</div>