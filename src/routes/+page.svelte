<script lang="ts">
    import type { SampleRecord, PaginatedResponse } from '$lib/types';
    import { parseTags, BASE_TAGS } from '$lib/utils/tags';
    import { getSemitoneShift, getStretchRatio } from '$lib/utils/audio';
    import { initDragDrop } from '$lib/utils/drag';

    import SettingsView from '$lib/components/settings/SettingsView.svelte';
    import MetadataEditor from '$lib/components/editor/MetadataEditor.svelte';
    import Pagination from '$lib/components/browser/Pagination.svelte';
    import BulkSidebar from '$lib/components/browser/BulkSidebar.svelte';
    import FilterMenu from '$lib/components/browser/FilterMenu.svelte';
    import SortMenu from '$lib/components/browser/SortMenu.svelte';
    import SampleRow from '$lib/components/browser/SampleRow.svelte';
    import SamplerModal from '$lib/components/editor/SamplerModal.svelte';
    import CollectionsGrid from '$lib/components/browser/CollectionsGrid.svelte';
    import EmptyLibrary from '$lib/components/browser/EmptyLibrary.svelte';
    import ScannerControls from '$lib/components/browser/ScannerControls.svelte';

    import { appState } from '$lib/store.svelte';
    import { initScannerListener } from '$lib/stores/scanner.svelte';
    import { editorState, openSampler } from '$lib/stores/editor.svelte';

    import { onMount, onDestroy, tick } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';

    let sortField: 'name' | 'type' | 'pack' | 'random' = $state('name');
    let sortOrder: 'asc' | 'desc' = $state('asc');

    let samples: SampleRecord[] = $state([]);
    let isLoading: boolean = $state(true);

    let hasActiveFilters = $derived(
        appState.globalSearchQuery.trim() !== '' ||
        appState.filters.instruments.length > 0 ||
        appState.filters.genres.length > 0 ||
        appState.filters.formats.length > 0 ||
        appState.filters.keys.length > 0 ||
        appState.filters.bpm.exact !== null ||
        appState.filters.bpm.min !== null ||
        appState.filters.bpm.max !== null ||
        appState.filters.onlyLiked ||
        appState.filters.collectionId !== null
    );

    let scrollContainer: HTMLDivElement | undefined = $state();

    let currentPage: number = $state(1);
    const pageSize: number = 50;
    let totalItems: number = $state(0);
    let totalPages: number = $derived(Math.ceil(totalItems / pageSize) || 1);

    let playingId: string | null = $state(null);
    let selectedId: string | null = $state(null);

    let currentSampleDuration: number = 0;
    let currentStretchRatio: number = 1.0;
    let playbackStartTime: number = 0;
    let animationFrameId: number;

    let availableTags = $state([...BASE_TAGS]);
    let allAvailableTags: Array<{category: string, value: string}> = $state([]);

    let activeDropdownTags = $derived.by(() => {
        if (appState.filters.tagMatchMode === 'OR') {
            const combined = [...BASE_TAGS, ...allAvailableTags];
            return combined.filter((tag, index, self) =>
                index === self.findIndex((t) => t.category === tag.category && t.value === tag.value)
            );
        } else {
            return availableTags;
        }
    });

    function updateProgress() {
        if (appState.isPlaying) {
            const elapsed = (performance.now() - playbackStartTime) / 1000;

            if (editorState.isOpen && editorState.sample) {
                const selectionDurationSec = ((editorState.sample.duration_ms * (editorState.trimEndPct - editorState.trimStartPct)) / 1000 / currentStretchRatio) / appState.vinylSpeedMode;

                if (elapsed >= selectionDurationSec) {
                    if (editorState.isLooping && editorState.currentPreviewPath) {
                        playbackStartTime = performance.now();
                        appState.playbackProgress = 0;
                        currentStretchRatio = getStretchRatio(editorState.sample.bpm, appState.globalBpm);

                        let semitones = 0;
                        if (appState.globalKey && editorState.sample.key_signature) {
                            semitones = getSemitoneShift(editorState.sample.key_signature, appState.globalKey, appState.globalKeyMode);
                        }

                        invoke('play_audio', { filePath: editorState.currentPreviewPath, semitones, stretchRatio: currentStretchRatio, volume: appState.globalVolume, speed: appState.vinylSpeedMode }).catch(console.error);
                        animationFrameId = requestAnimationFrame(updateProgress);
                    } else {
                        appState.playbackProgress = 1.0;
                        appState.isPlaying = false;
                        playingId = null;
                        invoke('stop_audio').catch(console.error);
                        cancelAnimationFrame(animationFrameId);
                        setTimeout(() => { if (!appState.isPlaying) appState.playbackProgress = 0; }, 150);
                    }
                } else {
                    appState.playbackProgress = elapsed / selectionDurationSec;
                    animationFrameId = requestAnimationFrame(updateProgress);
                }
            } else {
                if (currentSampleDuration > 0) {
                    let progress = elapsed / (currentSampleDuration / currentStretchRatio);
                    if (progress >= 1.0) {
                        appState.playbackProgress = 1.0;
                        appState.isPlaying = false;
                        playingId = null;
                        cancelAnimationFrame(animationFrameId);
                        setTimeout(() => { if (!appState.isPlaying) appState.playbackProgress = 0; }, 150);
                    } else {
                        appState.playbackProgress = progress;
                        animationFrameId = requestAnimationFrame(updateProgress);
                    }
                }
            }
        }
    }

    function handleSampleReload() {
        currentPage = 1;
        loadSamples();
    }

    const handleGlobalClick = () => {
        openContextMenuId = null;
    };

    onMount(async () => {
        initDragDrop();
        window.addEventListener('keydown', handleKeydown);
        window.addEventListener('click', handleGlobalClick);
        window.addEventListener('trigger-sample-reload', handleSampleReload);
        window.addEventListener('force-retrigger', handleForceRetrigger);

        try {
            await invoke<number>('cleanup_database');
            await loadSamples();
        } catch (e) { console.error(e); }

        await loadAllTags();
        await listen('library-updated', () => loadSamples());
        await initScannerListener();
    });

    onDestroy(() => {
        cancelAnimationFrame(animationFrameId);
        window.removeEventListener('keydown', handleKeydown);
        window.removeEventListener('click', handleGlobalClick);
        window.removeEventListener('trigger-sample-reload', handleSampleReload);
        window.removeEventListener('force-retrigger', handleForceRetrigger);
    });

    $effect(() => {
        invoke('set_audio_volume', { volume: appState.globalVolume });
    });

    let lastToggle = $state(0); let lastNext = $state(0); let lastPrev = $state(0);
    $effect(() => {
        if (appState.cmdTogglePlay > lastToggle) {
            lastToggle = appState.cmdTogglePlay;
            if (editorState.isOpen) {
                playSlicePreview();
                return;
            }
            if (appState.currentSample) {
                if (appState.isPlaying) {
                    invoke('stop_audio').catch(e => console.error(e));
                    appState.isPlaying = false; playingId = null; cancelAnimationFrame(animationFrameId);
                } else {
                    handlePlayRequest(appState.currentSample, true);
                }
            }
        }
        if (appState.cmdNext > lastNext) { lastNext = appState.cmdNext; playNextSample(); }
        if (appState.cmdPrev > lastPrev) { lastPrev = appState.cmdPrev; playPrevSample(); }
    });

    let searchTimeout: ReturnType<typeof setTimeout>;
    $effect(() => {
        const query = appState.globalSearchQuery;
        clearTimeout(searchTimeout);
        searchTimeout = setTimeout(() => {
            if (appState.currentView === 'sounds') {
                currentPage = 1;
                loadSamples();
            }
        }, 300);
    });

    async function toggleLike(sample: SampleRecord, event: Event) {
        event.stopPropagation();
        const originalState = sample.is_liked;
        const newState = !originalState;
        sample.is_liked = newState;

        if (appState.filters.onlyLiked && !newState) {
            samples = samples.filter(s => s.id !== sample.id);
            totalItems = Math.max(0, totalItems - 1);
        } else {
            samples = [...samples];
        }

        try {
            await invoke('toggle_sample_like', { id: sample.id, isLiked: newState });
        } catch (error) {
            if (appState.filters.onlyLiked && !newState) loadSamples();
            else { sample.is_liked = originalState; samples = [...samples]; }
        }
    }

    async function loadSamples() {
        isLoading = true;
        try {
            const response = await invoke<PaginatedResponse>('get_samples', {
                searchQuery: appState.globalSearchQuery.trim() !== '' ? appState.globalSearchQuery.trim() : null,
                page: currentPage,
                pageSize: pageSize,
                filters: $state.snapshot(appState.filters),
                sortField: sortField,
                sortOrder: sortOrder
            });
            samples = response.samples;
            totalItems = response.total_count;
            if (response.available_tags) availableTags = response.available_tags;

            if (scrollContainer) scrollContainer.scrollTop = 0;
            if (playingId === null && appState.isPlaying) {
                invoke('stop_audio').catch(console.error);
                appState.isPlaying = false;
            }
        } catch (error) { console.error(error); } finally { isLoading = false; }
    }

    async function playNextSample() {
        if (samples.length === 0) return;
        let currentIndex = samples.findIndex(s => s.id === selectedId);
        if (currentIndex === -1) currentIndex = 0;
        else if (currentIndex === samples.length - 1) {
            if (currentPage < totalPages) {
                currentPage++; await loadSamples();
                if (samples.length > 0) {
                    await handlePlayRequest(samples[0]);
                    setTimeout(() => scrollToSample(samples[0].id), 50);
                }
            }
            return;
        } else currentIndex++;
        await handlePlayRequest(samples[currentIndex]);
        setTimeout(() => scrollToSample(samples[currentIndex].id), 50);
    }

    async function playPrevSample() {
        if (samples.length === 0) return;
        let currentIndex = samples.findIndex(s => s.id === selectedId);
        if (currentIndex === -1) currentIndex = samples.length - 1;
        else if (currentIndex === 0) {
            if (currentPage > 1) {
                currentPage--; await loadSamples();
                if (samples.length > 0) {
                    await handlePlayRequest(samples[samples.length - 1]);
                    setTimeout(() => scrollToSample(samples[samples.length - 1].id), 50);
                }
            }
            return;
        } else currentIndex--;
        await handlePlayRequest(samples[currentIndex]);
        setTimeout(() => scrollToSample(samples[currentIndex].id), 50);
    }

    function scrollToSample(id: string) {
        const el = document.getElementById(`sample-${id}`);
        if (!el || !scrollContainer) return;
        const elRect = el.getBoundingClientRect();
        const containerRect = scrollContainer.getBoundingClientRect();
        if (elRect.top < containerRect.top) scrollContainer.scrollBy({ top: elRect.top - containerRect.top - 20, behavior: 'smooth' });
        else if (elRect.bottom > containerRect.bottom) scrollContainer.scrollBy({ top: elRect.bottom - containerRect.bottom + 20, behavior: 'smooth' });
    }

    async function handleKeydown(e: KeyboardEvent) {
        if (document.activeElement?.tagName === 'INPUT') return;

        if (editorState.isOpen) {
            if (['ArrowDown', 'ArrowUp', 'ArrowRight'].includes(e.key)) { e.preventDefault(); return; }
            if (e.key === 'ArrowLeft') { e.preventDefault(); playSlicePreview(true); return; }
            if (e.key === ' ') { e.preventDefault(); playSlicePreview(); return; }
            return;
        }

        if (isLoading) {
            if (['ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', ' '].includes(e.key)) e.preventDefault();
            return;
        }

        if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault();
            if (samples.length === 0) return;
            let currentIndex = samples.findIndex(s => s.id === selectedId);

            if (e.key === 'ArrowDown') {
                if (currentIndex === -1) currentIndex = 0;
                else if (currentIndex === samples.length - 1) {
                    if (currentPage < totalPages) {
                        currentPage++; await loadSamples();
                        if (samples.length > 0) {
                            if (appState.autoPlayEnabled !== false) await handlePlayRequest(samples[0]);
                            else selectedId = samples[0].id;
                            await tick(); scrollToSample(samples[0].id);
                        }
                    }
                    return;
                } else currentIndex++;
            } else if (e.key === 'ArrowUp') {
                if (currentIndex === -1) currentIndex = samples.length - 1;
                else if (currentIndex === 0) {
                    if (currentPage > 1) {
                        currentPage--; await loadSamples();
                        if (samples.length > 0) {
                            if (appState.autoPlayEnabled !== false) await handlePlayRequest(samples[samples.length - 1]);
                            else selectedId = samples[samples.length - 1].id;
                            await tick(); scrollToSample(samples[samples.length - 1].id);
                        }
                    }
                    return;
                } else currentIndex--;
            }

            const nextSample = samples[currentIndex];
            if (appState.autoPlayEnabled !== false) handlePlayRequest(nextSample);
            else selectedId = nextSample.id;

            await tick(); scrollToSample(nextSample.id);
        } else if (e.key === 'ArrowLeft') {
            e.preventDefault();
            if (!selectedId) return;
            const currentSample = samples.find(s => s.id === selectedId);
            if (currentSample) handlePlayRequest(currentSample, true);
        } else if (e.key === ' ') {
            e.preventDefault();
            if (!selectedId) return;
            const currentSample = samples.find(s => s.id === selectedId);
            if (currentSample) handlePlayRequest(currentSample);
        }
    }

    async function handlePlayRequest(sample: SampleRecord, forceRestart: boolean = false) {
        selectedId = sample.id;

        if (playingId === sample.id && !forceRestart) {
            await invoke('stop_audio');
            playingId = null;
            appState.isPlaying = false;
            appState.playbackProgress = 0;
            cancelAnimationFrame(animationFrameId);
            return;
        }

        cancelAnimationFrame(animationFrameId);
        playingId = sample.id;
        appState.playbackProgress = 0;
        appState.currentSample = sample;

        let semitones = 0;
        if (appState.globalKey && sample.key_signature) {
            semitones = getSemitoneShift(sample.key_signature, appState.globalKey, appState.globalKeyMode);
        }
        const stretchRatio = getStretchRatio(sample.bpm, appState.globalBpm);

        currentSampleDuration = (sample.duration_ms / 1000) / appState.vinylSpeedMode;
        currentStretchRatio = stretchRatio;

        try {
            await invoke('play_audio', {
                filePath: sample.original_path,
                semitones: semitones,
                stretchRatio: stretchRatio,
                volume: appState.globalVolume,
                speed: appState.vinylSpeedMode
            });

            appState.isPlaying = true;
            playbackStartTime = performance.now();
            animationFrameId = requestAnimationFrame(updateProgress);
        } catch (error) {
            console.error(error);
            playingId = null;
        }
    }

    async function playSlicePreview(forceRestart = false) {
        if (!editorState.sample) return;

        if (appState.isPlaying && !forceRestart) {
            invoke('stop_audio').catch(console.error);
            appState.isPlaying = false;
            cancelAnimationFrame(animationFrameId);
            return;
        }

        if (editorState.isSlicing && !editorState.currentPreviewPath) return;

        if (!editorState.isSliceReady && (!editorState.currentPreviewPath || editorState.previewSliceStartPct !== editorState.trimStartPct)) {
            editorState.isSlicing = true;
            try {
                const startMs = editorState.trimStartPct * editorState.sample.duration_ms;
                const endMs = editorState.sample.duration_ms;
                editorState.currentPreviewPath = await invoke<string>('slice_audio', {
                    path: editorState.sample.original_path,
                    startMs, endMs
                });
                editorState.previewSliceStartPct = editorState.trimStartPct;
            } catch (e) { console.error("Preview error:", e); return; }
            finally { editorState.isSlicing = false; }
        }

        if (editorState.currentPreviewPath) {
            let semitones = 0;
            if (appState.globalKey && editorState.sample.key_signature) {
                semitones = getSemitoneShift(editorState.sample.key_signature, appState.globalKey, appState.globalKeyMode);
            }
            try {
                if (appState.isPlaying) await invoke('stop_audio');
                await invoke('play_audio', {
                    filePath: editorState.currentPreviewPath,
                    semitones, stretchRatio: getStretchRatio(editorState.sample.bpm, appState.globalBpm),
                    volume: appState.globalVolume, speed: appState.vinylSpeedMode
                });
                appState.isPlaying = true;
                playbackStartTime = performance.now();
                animationFrameId = requestAnimationFrame(updateProgress);
            } catch (error) { console.error(error); }
        }
    }

    function handleForceRetrigger() {
        if (appState.currentSample && appState.isPlaying) {
            if (editorState.isOpen) playSlicePreview(true);
            else handlePlayRequest(appState.currentSample, true);
        }
    }

    function toggleSampleSelection(id: string, checked: boolean) {
        if (checked) {
            if (!appState.selectedSampleIds.includes(id)) appState.selectedSampleIds.push(id);
        } else {
            appState.selectedSampleIds = appState.selectedSampleIds.filter(sId => sId !== id);
        }
    }

    let openContextMenuId: string | null = $state(null);
    let editingSample: SampleRecord | null = $state(null);

    function openEditModal(sample: SampleRecord) {
        editingSample = sample;
        openContextMenuId = null;
    }

    async function loadAllTags() {
        try {
            allAvailableTags = await invoke('get_all_available_tags');
        } catch (e) { console.error(e); }
    }
</script>

{#if appState.currentView === 'sounds'}
    <div class="flex h-full w-full overflow-hidden relative">
        <div class="flex-1 overflow-y-auto overflow-x-clip transition-all duration-300 {appState.selectedSampleIds.length > 0 ? 'mr-72' : 'mr-0'}" bind:this={scrollContainer}>

            <div class="pl-8 pr-8 pt-8 pb-2 w-full">
                <div class="mb-6 flex items-end justify-between border-b border-zinc-200 pb-0 dark:border-zinc-800">
                    <div class="flex-1">
                        <div class="flex items-center gap-6 mb-4">
                            <h1 class="text-3xl font-bold tracking-tight">
                                {#if appState.filters.onlyLiked} Liked
                                {:else if appState.filters.collectionId} {appState.collections.find(c => c.id === appState.filters.collectionId)?.name || 'Collection'}
                                {:else} Sounds {/if}
                            </h1>
                        </div>
                        <div class="flex gap-6">
                            <button onclick={() => { appState.activeSoundsTab = 'samples'; appState.filters.collectionId = null; appState.filters.onlyLiked = false; currentPage = 1; loadSamples(); }} class="pb-2 text-sm font-semibold transition-colors cursor-pointer {appState.activeSoundsTab === 'samples' ? 'border-b-2 border-zinc-900 text-zinc-900 dark:border-zinc-100 dark:text-zinc-100' : 'text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 border-b-2 border-transparent'}">Samples</button>
                            <button onclick={() => { appState.activeSoundsTab = 'collections'; appState.filters.collectionId = null; appState.filters.onlyLiked = false; }} class="pb-2 text-sm font-semibold transition-colors cursor-pointer {appState.activeSoundsTab === 'collections' ? 'border-b-2 border-zinc-900 text-zinc-900 dark:border-zinc-100 dark:text-zinc-100' : 'text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 border-b-2 border-transparent'}">Collections</button>
                        </div>
                    </div>
                    <ScannerControls loadSamples={() => { currentPage = 1; loadSamples(); }} />
                </div>
            </div>

            {#if appState.activeSoundsTab === 'collections' && appState.filters.collectionId === null && !appState.filters.onlyLiked}
                <div class="pl-8 pr-8 pb-8 pt-4 w-full">
                    <CollectionsGrid bind:currentPage {loadSamples} />
                </div>
            {:else}
                <div class="pb-8">
                    <div class="pl-8 pr-8 w-full">
                        <FilterMenu
                                {activeDropdownTags}
                                {availableTags}
                                loadSamples={() => { currentPage = 1; loadSamples(); }}
                        />

                        <div class="mb-4 flex items-center justify-between text-sm text-zinc-500 relative">
                            <span>{#if isLoading}Loading...{:else}{totalItems} results{/if}</span>
                            <div class="flex items-center gap-3">
                                <span class="text-xs font-medium">Sort by:</span>
                                <SortMenu bind:sortField bind:sortOrder loadSamples={() => { currentPage = 1; loadSamples(); }} />
                            </div>
                        </div>
                    </div>

                    <div class="w-full overflow-x-auto pb-4 no-scrollbar">
                        <div class="min-w-[760px] pl-8 pr-8">
                            <div class="grid grid-cols-[20px_40px_32px_minmax(150px,2fr)_minmax(120px,1.5fr)_50px_40px_40px_32px_32px] gap-4 border-y border-zinc-200 py-2 text-[11px] font-semibold uppercase tracking-wider text-zinc-500 dark:border-zinc-800 items-center -mx-2 px-2">
                                <div></div><div></div><div></div>
                                <div>Filename</div><div>Waveform</div><div class="text-right">Time</div><div class="text-center">Key</div><div class="text-center">BPM</div>
                                <div></div><div></div>
                            </div>

                            <div class="relative mt-2">
                                {#if isLoading && samples.length > 0}
                                    <div class="absolute inset-x-0 top-10 z-10 flex justify-center pointer-events-none"><div class="rounded-full bg-white/90 px-4 py-2 text-xs font-semibold text-zinc-900 shadow-sm backdrop-blur-md dark:bg-zinc-800/90 dark:text-zinc-100">Filtering...</div></div>
                                {/if}

                                {#if isLoading && samples.length === 0}
                                    <div class="flex justify-center items-center h-40 text-sm text-zinc-500 animate-pulse">Loading library...</div>
                                {:else if samples.length === 0}
                                    <EmptyLibrary
                                            {hasActiveFilters}
                                            clearAllFilters={() => {
                                                appState.filters.instruments = []; appState.filters.genres = []; appState.filters.formats = []; appState.filters.keys = [];
                                                appState.filters.bpm.exact = null; appState.filters.bpm.min = null; appState.filters.bpm.max = null;
                                                appState.filters.tagMatchMode = 'AND';
                                                currentPage = 1; loadSamples();
                                            }}
                                            loadSamples={() => { currentPage = 1; loadSamples(); }}
                                    />
                                {:else}
                                    <div class="divide-y divide-zinc-100 dark:divide-zinc-800/50 mb-8 transition-opacity duration-200 {isLoading ? 'opacity-40 pointer-events-none' : 'opacity-100'}">
                                        {#each samples as sample}
                                            <SampleRow
                                                    {sample}
                                                    {selectedId}
                                                    {playingId}
                                                    bind:openContextMenuId
                                                    {handlePlayRequest}
                                                    {toggleLike}
                                                    {openEditModal}
                                                    openSampler={() => openSampler(sample)}
                                                    {toggleSampleSelection}
                                            />
                                        {/each}
                                    </div>
                                {/if}
                            </div>
                        </div>
                    </div>

                    <Pagination bind:currentPage {totalPages} {loadSamples} />
                </div>
            {/if}
        </div>

        <BulkSidebar bind:samples {allAvailableTags} {loadAllTags} />
    </div>
{:else if appState.currentView === 'projects'}
    <div class="flex h-full items-center justify-center text-zinc-500"><h2 class="text-2xl font-bold">Musik Projekte (Coming Soon)</h2></div>
{:else if appState.currentView === 'editor'}
    <div class="flex h-full w-full flex-col overflow-y-auto px-10 py-8"></div>
{:else if appState.currentView === 'settings'}
    <SettingsView bind:samples {allAvailableTags} {loadAllTags} />
{/if}

{#if editingSample}
    <MetadataEditor bind:editingSample bind:samples {allAvailableTags} {loadAllTags} />
{/if}

{#if editorState.isOpen}
    <SamplerModal {playSlicePreview} />
{/if}