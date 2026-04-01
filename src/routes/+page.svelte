<script lang="ts">
    import { onMount, onDestroy, tick } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { open } from '@tauri-apps/plugin-dialog';
    import { EllipsisVertical, Download, Heart, Play, Pause, FolderPlus, RefreshCw, Trash2, Image as ImageIcon, ChevronLeft, ChevronRight, Settings, X, ChevronDown, ArrowDownUp, Shuffle, Folder, Plus } from 'lucide-svelte';
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
        tags: string;
        is_liked: boolean;
    };

    function parseTags(tagsJson: string) {
        try {
            return JSON.parse(tagsJson);
        } catch {
            return [];
        }
    }

    type PaginatedResponse = {
        samples: SampleRecord[];
        total_count: number;
        available_tags: Array<{category: string, value: string}>;
    };

    // --- SORT STATE ---
    let sortField: 'name' | 'type' | 'pack' | 'random' = $state('name');
    let sortOrder: 'asc' | 'desc' = $state('asc');
    let isSortDropdownOpen = $state(false);

    let samples: SampleRecord[] = $state([]);
    let isLoading: boolean = $state(true);

    let selectedPath: string | null = $state(null);
    let isSyncing: boolean = $state(false);
    let isScanningNew: boolean = $state(false);
    let isClearing: boolean = $state(false);
    let isScanning = $derived(isSyncing || isScanningNew || isClearing);
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
    let gainNode: GainNode;
    let sourceNode: AudioBufferSourceNode | null = null;
    let playingId: string | null = $state(null);
    let selectedId: string | null = $state(null);

    const audioRamCache = new Map<string, AudioBuffer>();

    let playbackProgress: number = $state(0);
    let currentSampleDuration: number = 0;
    let playbackStartTime: number = 0;
    let animationFrameId: number;

    // --- TAGS LOGIC (Richtig sortiert für den Compiler) ---
    let availableTags: Array<{category: string, value: string}> = $state([
        { category: 'Drums', value: 'Drums' },
        { category: 'Drums', value: 'Kick' },
        { category: 'Drums', value: 'Snare' },
        { category: 'Percussion', value: 'Percussion' },
        { category: 'Synth', value: 'Synth' },
        { category: 'Bass', value: '808' },
        { category: 'Genre', value: 'Trap' },
        { category: 'Genre', value: 'Afrobeats' },
        { category: 'Genre', value: 'House' },
        { category: 'Format', value: 'One-Shot' },
        { category: 'Format', value: 'Loop' },
    ]);

    let userTags: Array<{category: string, value: string}> = $state([]);

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
            playbackProgress = progress;
            appState.playbackProgress = progress;

            if (progress >= 1.0) {
                cancelAnimationFrame(animationFrameId);
            } else {
                animationFrameId = requestAnimationFrame(updateProgress);
            }
        }
    }

    function handleSampleReload() {
        currentPage = 1;
        loadSamples();
    }

    async function loadUserTags() {
        try {
            userTags = await invoke('get_user_tags');
        } catch (e) { console.error(e); }
    }

    // Globaler Click Handler (Muss VOR onMount definiert sein)
    const handleGlobalClick = (e: MouseEvent) => {
        openDropdown = null;
        isSortDropdownOpen = false;
        openContextMenuId = null;
        if (isTagDropdownOpen) {
            const target = e.target as HTMLElement;
            if (!target.closest('.tag-dropdown-container')) {
                isTagDropdownOpen = false;
            }
        }
    };

    onMount(async () => {
        // @ts-ignore
        const AudioContextClass = window.AudioContext || window.webkitAudioContext;
        audioCtx = new AudioContextClass();

        gainNode = audioCtx.createGain();
        gainNode.connect(audioCtx.destination);
        gainNode.gain.value = appState.globalVolume;

        window.addEventListener('keydown', handleKeydown);
        window.addEventListener('click', handleGlobalClick);
        window.addEventListener('trigger-sample-reload', handleSampleReload);

        try {
            await invoke<number>('cleanup_database');
            await loadSamples();
        } catch (e) { console.error(e); }

        await loadAllTags();

        await listen('library-updated', () => loadSamples());
        await listen<ScanProgressPayload>('scan-progress', (e) => {
            scanTotal = e.payload.total; scanCurrent = e.payload.current; scanCurrentFile = e.payload.current_file;
        });
    });

    onDestroy(() => {
        if (sourceNode) sourceNode.stop();
        if (audioCtx) audioCtx.close();
        cancelAnimationFrame(animationFrameId);

        window.removeEventListener('keydown', handleKeydown);
        window.removeEventListener('click', handleGlobalClick);
        window.removeEventListener('trigger-sample-reload', handleSampleReload);
    });

    $effect(() => {
        if (gainNode) gainNode.gain.value = appState.globalVolume;
    });

    async function createNewTag() {
        const trimmed = tagSearchQuery.trim();
        if (trimmed === '') return;
        try {
            await invoke('create_user_tag', { category: 'User', value: trimmed });
            await loadAllTags();
            addTagToEditor('User', trimmed);
        } catch (e) { console.error(e); }
    }

    async function handleDeleteUserTag(value: string, event: Event) {
        event.stopPropagation();
        try {
            // 1. Backend: Tag komplett aus der DB und allen Samples löschen
            await invoke('delete_user_tag', { value });

            // 2. Frontend: Dropdown-Lexikon aktualisieren
            await loadAllTags();

            // 3. Frontend: Tag sofort aus dem aktuellen Editor-Fenster werfen
            editForm.tags = editForm.tags.filter(t => t.value !== value);

            // 4. Frontend: Tag live aus der Hauptliste im Hintergrund entfernen
            samples = samples.map(sample => {
                let parsed = parseTags(sample.tags);
                let filtered = parsed.filter((t: {category: string, value: string}) => t.value !== value);

                // Nur updaten, wenn der Tag in diesem Sample wirklich existierte
                if (parsed.length !== filtered.length) {
                    sample.tags = JSON.stringify(filtered);
                }
                return sample;
            });
        } catch (e) {
            console.error(e);
        }
    }

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
            if(response.available_tags) availableTags = response.available_tags;

            if (scrollContainer) scrollContainer.scrollTop = 0;
            if (sourceNode) { sourceNode.stop(); playingId = null; }
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
                    setTimeout(() => document.getElementById(`sample-${samples[0].id}`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' }), 50);
                }
            }
            return;
        } else currentIndex++;

        await handlePlayRequest(samples[currentIndex]);
        setTimeout(() => document.getElementById(`sample-${samples[currentIndex].id}`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' }), 50);
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
                    setTimeout(() => document.getElementById(`sample-${samples[samples.length - 1].id}`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' }), 50);
                }
            }
            return;
        } else currentIndex--;

        await handlePlayRequest(samples[currentIndex]);
        setTimeout(() => document.getElementById(`sample-${samples[currentIndex].id}`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' }), 50);
    }

    async function handleKeydown(e: KeyboardEvent) {
        if (document.activeElement?.tagName === 'INPUT') return;
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
                            await handlePlayRequest(samples[0]);
                            await tick();
                            document.getElementById(`sample-${samples[0].id}`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
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
                            await handlePlayRequest(samples[samples.length - 1]);
                            await tick();
                            document.getElementById(`sample-${samples[samples.length - 1].id}`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
                        }
                    }
                    return;
                } else currentIndex--;
            }

            const nextSample = samples[currentIndex];
            handlePlayRequest(nextSample);
            await tick();
            document.getElementById(`sample-${nextSample.id}`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });

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

    // --- ENTERPRISE PITCH CALCULATION ---
    const CHROMATIC_SCALE = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];

    function getSemitoneShift(sampleKey: string | null, targetKey: string | null): number {
        if (!sampleKey || !targetKey) return 0;

        // Extrahiere nur die reine Note (Ignoriere vorerst 'min' oder 'maj')
        const sNote = sampleKey.split(' ')[0].toUpperCase();
        const tNote = targetKey.split(' ')[0].toUpperCase();

        const sIdx = CHROMATIC_SCALE.indexOf(sNote);
        const tIdx = CHROMATIC_SCALE.indexOf(tNote);

        if (sIdx === -1 || tIdx === -1) return 0;

        let diff = tIdx - sIdx;

        // Shortest Path Logic: Niemals mehr als 6 Halbtöne pitchen
        if (diff > 6) diff -= 12;
        if (diff < -6) diff += 12;

        return diff;
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
        appState.playbackProgress = 0;

        // 1. Berechne den nötigen Shift
        let semitones = 0;
        if (appState.globalKey && sample.key_signature) {
            semitones = getSemitoneShift(sample.key_signature, appState.globalKey);
        }

        // 2. Erzeuge den Cache-Key inklusive Pitch! (Sonst lädt Svelte immer das alte Original aus dem RAM)
        const cacheKey = `${sample.id}_${semitones}`;
        let audioBuffer = audioRamCache.get(cacheKey);

        if (!audioBuffer) {
            try {
                // 3. Rust um den (gepitchten) Dateipfad bitten
                const pathToLoad = await invoke<string>('process_audio_pitch', {
                    filePath: sample.original_path,
                    semitones: semitones
                });

                // 4. Pfad laden (entweder das Original oder die unsichtbare Cache-Datei von Rust)
                const assetUrl = convertFileSrc(pathToLoad);
                const response = await fetch(assetUrl);
                audioBuffer = await audioCtx.decodeAudioData(await response.arrayBuffer());

                if (audioRamCache.size >= 200) { const firstKey = audioRamCache.keys().next().value; if (firstKey) audioRamCache.delete(firstKey); }
                audioRamCache.set(cacheKey, audioBuffer);
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
                appState.playbackProgress = 1.0;
                playingId = null;
                appState.isPlaying = false;
                cancelAnimationFrame(animationFrameId);
                setTimeout(() => {
                    if (!appState.isPlaying) appState.playbackProgress = 0;
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
    function parseWaveform(data: number[] | null): number[] { return (!data || data.length === 0) ? Array(40).fill(10) : data; }

    async function handleSelectFolder() {
        try {
            const result = await open({ directory: true, multiple: false });
            if (result) { selectedPath = result as string; scanMessage = 'Ready to scan.'; }
        } catch (error) { console.error(error); }
    }

    async function handleScan() {
        if (!selectedPath) return;
        isScanningNew = true;
        scanMessage = 'Indexing...';
        try {
            const count = await invoke<number>('scan_library', { path: selectedPath });
            scanMessage = `Added ${count} files.`;
            selectedPath = null;
            currentPage = 1;
            await loadSamples();
        } catch (error) { scanMessage = `Error: ${error}`; }
        finally { isScanningNew = false; }
    }

    async function handleClearDatabase() {
        if (confirm("Clear the entire library? This action cannot be undone.")) {
            isClearing = true;
            try {
                await invoke('clear_database');
                activeTypeFilter = null;
                currentPage = 1;
                samples = [];
                totalItems = 0;
                appState.collections = [];
                appState.filters.collectionId = null;
                scanMessage = 'Library cleared.';
            } catch (error) { scanMessage = `Error: ${error}`; }
            finally { isClearing = false; }
        }
    }

    async function handleRescanAll() {
        isSyncing = true;
        scanMessage = 'Syncing folders...';
        try {
            const removed = await invoke<number>('cleanup_database');
            const added = await invoke<number>('rescan_all_folders');
            scanMessage = `Synced! Added: ${added}, Removed: ${removed}`;
            currentPage = 1;
            await loadSamples();
            setTimeout(() => { scanMessage = ''; }, 3000);
        } catch (error) { scanMessage = `Error: ${error}`; }
        finally { isSyncing = false; }
    }

    function formatDuration(ms: number): string {
        if (ms === 0) return "--:--";
        const totalSec = Math.floor(ms / 1000);
        return `${Math.floor(totalSec / 60)}:${(totalSec % 60).toString().padStart(2, '0')}`;
    }

    // --- FILTER UI STATE ---
    let openDropdown: 'instrument' | 'genre' | 'key' | 'bpm' | 'format' | null = $state(null);
    const whiteKeys = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
    const blackKeys = [
        { note: 'C#', left: '14.28%' }, { note: 'D#', left: '28.56%' }, { note: 'F#', left: '57.12%' }, { note: 'G#', left: '71.40%' }, { note: 'A#', left: '85.68%' }
    ];

    let activeKeyFilter = $derived(appState.filters.keys.length > 0 ? appState.filters.keys[0] : null);
    let currentKeyMode = $derived.by(() => {
        if (!activeKeyFilter) return null;
        if (activeKeyFilter === 'min' || activeKeyFilter.endsWith(' min')) return 'min';
        if (activeKeyFilter === 'maj' || activeKeyFilter.endsWith(' maj')) return 'maj';
        return null;
    });
    let currentBaseNote = $derived.by(() => {
        if (!activeKeyFilter) return null;
        if (activeKeyFilter === 'min' || activeKeyFilter === 'maj') return null;
        return activeKeyFilter.split(' ')[0];
    });

    function togglePianoKey(note: string) {
        if (currentBaseNote === note) {
            if (currentKeyMode) appState.filters.keys = [currentKeyMode];
            else appState.filters.keys = [];
        } else {
            if (currentKeyMode) appState.filters.keys = [`${note} ${currentKeyMode}`];
            else appState.filters.keys = [note];
        }
        currentPage = 1; loadSamples();
    }

    function switchKeyMode(mode: 'min' | 'maj') {
        if (currentKeyMode === mode) {
            if (currentBaseNote) appState.filters.keys = [currentBaseNote];
            else appState.filters.keys = [];
        } else {
            if (currentBaseNote) appState.filters.keys = [`${currentBaseNote} ${mode}`];
            else appState.filters.keys = [mode];
        }
        currentPage = 1; loadSamples();
    }

    function isPianoKeyActive(note: string) { return currentBaseNote === note; }
    function toggleTagMatchMode() { appState.filters.tagMatchMode = appState.filters.tagMatchMode === 'AND' ? 'OR' : 'AND'; currentPage = 1; loadSamples(); }

    let isTagsExpanded: boolean = $state(false);
    let sortedAvailableTags = $derived.by(() => {
        return [...availableTags].sort((a, b) => {
            const aActive = isTagActive(a.category, a.value) ? 1 : 0;
            const bActive = isTagActive(b.category, b.value) ? 1 : 0;
            if (aActive !== bActive) return bActive - aActive;
            return 0;
        });
    });

    function toggleDropdown(dropdown: 'instrument' | 'genre' | 'key' | 'bpm' | 'format' | 'globalkey', event: Event) {
        event.stopPropagation();
        openDropdown = openDropdown === dropdown ? null : dropdown;
    }

    function toggleFilterTag(category: string, value: string) {
        let targetArray: string[];
        if (category === 'Genre') targetArray = appState.filters.genres;
        else if (category === 'Format') targetArray = appState.filters.formats;
        else if (category === 'Key') targetArray = appState.filters.keys;
        else targetArray = appState.filters.instruments;

        const idx = targetArray.indexOf(value);
        if (idx > -1) targetArray.splice(idx, 1);
        else targetArray.push(value);

        if (category === 'Genre') appState.filters.genres = [...targetArray];
        else if (category === 'Format') appState.filters.formats = [...targetArray];
        else if (category === 'Key') appState.filters.keys = [...targetArray];
        else appState.filters.instruments = [...targetArray];

        currentPage = 1; loadSamples();
    }

    function isTagActive(category: string, value: string): boolean {
        if (category === 'Genre') return appState.filters.genres.includes(value);
        if (category === 'Format') return appState.filters.formats.includes(value);
        if (category === 'Key') return appState.filters.keys.includes(value);
        return appState.filters.instruments.includes(value);
    }

    function clearAllFilters() {
        appState.filters.instruments = []; appState.filters.genres = []; appState.filters.formats = []; appState.filters.keys = [];
        appState.filters.bpm.exact = null; appState.filters.bpm.min = null; appState.filters.bpm.max = null;
        appState.filters.tagMatchMode = 'AND';
        currentPage = 1; loadSamples();
    }

    // --- SETTINGS MODAL LOGIC ---
    let connectedFolders: string[] = $state([]);
    let isSettingsLoading: boolean = $state(false);

    function setThemePref(pref: 'light' | 'dark' | 'system') {
        appState.themePreference = pref;
        localStorage.setItem('samplevault-theme', pref);
    }

    $effect(() => { if (appState.currentView === 'settings') loadConnectedFolders(); });

    async function loadConnectedFolders() {
        isSettingsLoading = true;
        try { connectedFolders = await invoke<string[]>('get_connected_folders'); }
        catch (error) { console.error(error); }
        finally { isSettingsLoading = false; }
    }

    async function handleRemoveFolder(folderPath: string) {
        if (!confirm(`Un-link this folder?\n\n${folderPath}\n\nThis will remove all its samples from your library.`)) return;
        isSettingsLoading = true;
        try {
            await invoke('remove_folder', { path: folderPath });
            await loadConnectedFolders();
            currentPage = 1; await loadSamples();
        } catch (error) { console.error(error); }
        finally { isSettingsLoading = false; }
    }

    function toggleSampleSelection(id: string, checked: boolean) {
        if (checked) {
            if (!appState.selectedSampleIds.includes(id)) appState.selectedSampleIds.push(id);
        } else {
            appState.selectedSampleIds = appState.selectedSampleIds.filter(sId => sId !== id);
        }
    }

    async function handleBulkAddToCollection(collectionId: number) {
        const ids = [...appState.selectedSampleIds];
        appState.selectedSampleIds = [];
        try { await invoke('add_to_collection', { collectionId, sampleIds: ids }); }
        catch (e) { console.error(e); }
    }

    async function handleBulkLike() {
        const ids = [...appState.selectedSampleIds];
        samples.forEach(s => { if (ids.includes(s.id)) s.is_liked = true; });
        samples = [...samples];
        appState.selectedSampleIds = [];
        try { await invoke('bulk_toggle_like', { sampleIds: ids, isLiked: true }); }
        catch (e) { console.error(e); }
    }

    // --- CONTEXT MENU & EDITOR STATE ---
    let openContextMenuId: string | null = $state(null);
    let editingSample: SampleRecord | null = $state(null);
    let editForm = $state({ filename: '', bpm: null as number | null, key_signature: '', tags: [] as Array<{category: string, value: string}> });
    let isTagDropdownOpen: boolean = $state(false);

    function openEditModal(sample: SampleRecord) {
        editingSample = sample;
        editForm = { filename: sample.filename, bpm: sample.bpm, key_signature: sample.key_signature || '', tags: parseTags(sample.tags) };
        openContextMenuId = null; isTagDropdownOpen = false; tagSearchQuery = '';
    }

    function removeTagFromEditor(index: number) {
        editForm.tags.splice(index, 1);
        editForm.tags = [...editForm.tags];
    }

    function addTagToEditor(category: string, value: string) {
        if (!editForm.tags.some(t => t.value === value && t.category === category)) {
            editForm.tags.push({ category, value });
            editForm.tags = [...editForm.tags];
        }
        isTagDropdownOpen = false; tagSearchQuery = '';
    }

    async function saveMetadata() {
        if (!editingSample) return;
        try {
            const tagsJson = JSON.stringify(editForm.tags);
            await invoke('update_sample_metadata', {
                payload: {
                    id: editingSample.id, filename: editForm.filename.trim(), bpm: editForm.bpm,
                    keySignature: editForm.key_signature.trim() !== '' ? editForm.key_signature.trim() : null, tags: tagsJson
                }
            });

            const index = samples.findIndex(s => s.id === editingSample!.id);
            if (index !== -1) {
                samples[index].filename = editForm.filename.trim(); samples[index].bpm = editForm.bpm;
                samples[index].key_signature = editForm.key_signature.trim() !== '' ? editForm.key_signature.trim() : null;
                samples[index].tags = tagsJson;
            }
            samples = [...samples]; editingSample = null;
        } catch(e) { console.error("Failed to save metadata:", e); }
    }

    // --- TAGS LOGIC ---
    let allAvailableTags: Array<{category: string, value: string}> = $state([]);
    let tagSearchQuery: string = $state('');

    let filteredTagsForEditor = $derived(
        allAvailableTags.filter(t => t.value.toLowerCase().includes(tagSearchQuery.toLowerCase()))
    );

    // Diese Funktion lädt nun das KOMPLETTE Lexikon (System + User) aus Rust
    async function loadAllTags() {
        try {
            allAvailableTags = await invoke('get_all_available_tags');
        } catch (e) { console.error(e); }
    }
</script>

{#if appState.currentView === 'sounds'}
    <div class="flex h-full w-full overflow-hidden">
        <div class="flex-1 overflow-y-auto transition-all duration-300" bind:this={scrollContainer}>
            <div class="px-8 pt-8 pb-2">
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

                        <div class="mt-4 flex items-center gap-2">
                            <span class="text-[10px] font-bold uppercase tracking-wider text-zinc-400">Project Key:</span>
                            <div class="relative">
                                <button
                                        onclick={(e) => { e.stopPropagation(); openDropdown = openDropdown === 'globalkey' ? null : 'globalkey'; }}
                                        class="flex h-7 items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 text-xs font-bold transition-colors cursor-pointer {appState.globalKey ? 'border-emerald-500 text-emerald-600 bg-emerald-50 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-400' : 'text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800'}"
                                >
                                    {appState.globalKey || 'Off'}
                                    <ChevronDown size={14} class="opacity-50" />
                                </button>

                                {#if openDropdown === 'globalkey'}
                                    <div onclick={(e) => e.stopPropagation()} class="absolute left-0 top-full mt-1 w-48 flex-col rounded-lg border border-zinc-200 bg-white p-2 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                                        <button
                                                onclick={() => { appState.globalKey = null; openDropdown = null; }}
                                                class="w-full text-left rounded-md px-2 py-1.5 text-xs font-semibold text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-zinc-100 cursor-pointer transition-colors mb-1"
                                        >
                                            Turn Off
                                        </button>
                                        <div class="grid grid-cols-4 gap-1">
                                            {#each CHROMATIC_SCALE as note}
                                                <button
                                                        onclick={() => { appState.globalKey = note; openDropdown = null; }}
                                                        class="flex items-center justify-center rounded border border-zinc-200 bg-zinc-50 py-1.5 text-xs font-bold text-zinc-700 hover:border-emerald-500 hover:text-emerald-600 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:border-emerald-500 dark:hover:text-emerald-400 transition-all cursor-pointer {appState.globalKey === note ? 'border-emerald-500 bg-emerald-50 text-emerald-600 dark:border-emerald-500/50 dark:bg-emerald-900/30 dark:text-emerald-400' : ''}"
                                                >
                                                    {note}
                                                </button>
                                            {/each}
                                        </div>
                                    </div>
                                {/if}
                            </div>

                            {#if appState.globalKey}
                                <span class="text-[10px] text-emerald-600/70 dark:text-emerald-400/70 ml-1 font-medium italic animate-pulse">Auto-Pitch active</span>
                            {/if}
                        </div>
                    </div>
                    <div class="flex flex-col items-end gap-3 pb-2">
                        {#if isScanning && scanTotal > 0}
                            <div class="w-full flex flex-col gap-1 mt-1">
                                <div class="flex justify-between text-[10px] font-medium text-zinc-500 uppercase tracking-wider">
                                    <span>Scanning: {scanCurrent} / {scanTotal}</span><span>{scanPercentage}%</span>
                                </div>
                                <div class="h-1.5 w-full rounded-full bg-zinc-200 overflow-hidden dark:bg-zinc-800">
                                    <div class="h-full bg-blue-500 transition-all duration-300 ease-out" style="width: {scanPercentage}%"></div>
                                </div>
                                <div class="text-[10px] text-zinc-400 truncate text-right">{scanCurrentFile}</div>
                            </div>
                        {:else if scanMessage}
                            <span class="text-xs font-medium text-zinc-500 animate-pulse">{scanMessage}</span>
                        {/if}
                        <div class="flex items-center gap-2">
                            <button onclick={handleRescanAll} disabled={isScanning} class="flex items-center gap-1.5 rounded-md border border-blue-700/50 bg-blue-50 px-3 py-1.5 text-xs font-medium text-blue-700 hover:bg-blue-100 dark:border-blue-500/30 dark:bg-blue-500/10 dark:text-blue-400 cursor-pointer disabled:opacity-50"><RefreshCw size={14} class={isSyncing ? "animate-spin" : ""} /> Sync</button>
                            <button onclick={handleSelectFolder} disabled={isScanning} class="flex items-center gap-1.5 rounded-md border border-zinc-200 bg-white px-3 py-1.5 text-xs font-medium hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-800 dark:hover:bg-zinc-700 transition-colors disabled:opacity-50 cursor-pointer"><FolderPlus size={14} /> Add Folder</button>
                            {#if selectedPath}
                                <button onclick={handleScan} disabled={isScanning} class="flex items-center gap-1.5 rounded-md border border-emerald-700/50 bg-emerald-50 px-3 py-1.5 text-xs font-medium text-emerald-700 hover:bg-emerald-100 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-400 cursor-pointer disabled:opacity-50">
                                    {#if isScanningNew} <RefreshCw size={14} class="animate-spin" /> Scanning... {:else} <Play size={14} /> Scan New {/if}
                                </button>
                            {/if}
                        </div>
                    </div>
                </div>
            </div>

            {#if appState.activeSoundsTab === 'collections' && appState.filters.collectionId === null && !appState.filters.onlyLiked}
                <div class="px-8 pb-8 pt-4">
                    <div class="grid grid-cols-2 gap-6 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
                        <button onclick={() => { appState.filters.onlyLiked = true; currentPage = 1; loadSamples(); }} class="group flex aspect-square cursor-pointer flex-col justify-between text-left transition-all hover:-translate-y-1">
                            <div class="flex h-full w-full flex-col items-center justify-center rounded-2xl border border-red-100 bg-gradient-to-br from-red-50 to-red-100/50 shadow-sm transition-all group-hover:shadow-md dark:border-red-900/30 dark:from-red-950/40 dark:to-red-900/10">
                                <Heart size={48} class="fill-red-500 text-red-500 transition-transform group-hover:scale-110" />
                                <span class="mt-4 font-bold text-red-900 dark:text-red-400">Liked Sounds</span>
                            </div>
                        </button>
                        {#each appState.collections as collection}
                            <button onclick={() => { appState.filters.collectionId = collection.id; currentPage = 1; loadSamples(); }} class="group flex aspect-square cursor-pointer flex-col justify-between rounded-2xl border border-zinc-200 bg-white p-5 text-left shadow-sm transition-all hover:-translate-y-1 hover:shadow-md dark:border-zinc-800 dark:bg-[#18181b]">
                                <div class="flex h-12 w-12 items-center justify-center rounded-full bg-zinc-100 text-zinc-600 transition-colors group-hover:bg-zinc-900 group-hover:text-white dark:bg-zinc-800 dark:text-zinc-400 dark:group-hover:bg-zinc-100 dark:group-hover:text-zinc-900"><Folder size={24} /></div>
                                <div>
                                    <h3 class="truncate text-lg font-bold text-zinc-900 dark:text-zinc-100" title={collection.name}>{collection.name}</h3>
                                    <p class="mt-1 text-xs text-zinc-500">Collection</p>
                                </div>
                            </button>
                        {/each}
                        <button onclick={() => appState.isCreateCollectionModalOpen = true} class="group flex aspect-square cursor-pointer flex-col items-center justify-center gap-3 rounded-2xl border-2 border-dashed border-zinc-200 bg-zinc-50/50 p-8 text-zinc-500 transition-all hover:border-zinc-300 hover:bg-zinc-100 hover:text-zinc-900 dark:border-zinc-800 dark:bg-[#18181b]/50 dark:hover:border-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-100">
                            <Plus size={32} class="transition-transform group-hover:scale-110" />
                            <span class="text-sm font-bold">New Collection</span>
                        </button>
                    </div>
                </div>
            {:else}
                <div class="px-8 pb-8">
                    <div class="mb-6 space-y-3">
                        <div class="flex flex-wrap items-center gap-2">
                            <div class="relative">
                                <button onclick={(e) => toggleDropdown('instrument', e)} class="flex h-8 items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 text-xs font-semibold transition-colors cursor-pointer {appState.filters.instruments.length > 0 ? 'border-zinc-900 dark:border-zinc-100 text-zinc-900 dark:text-zinc-100' : 'text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800'}">
                                    Instrument {#if appState.filters.instruments.length > 0}<span class="flex h-4 w-4 items-center justify-center rounded-full bg-zinc-900 text-[9px] text-white dark:bg-zinc-100 dark:text-zinc-900">{appState.filters.instruments.length}</span>{/if}<ChevronDown size={14} class="opacity-50" />
                                </button>
                                <div onclick={(e) => e.stopPropagation()} class="absolute left-0 top-full mt-1 {openDropdown === 'instrument' ? 'flex' : 'hidden'} w-48 flex-col rounded-lg border border-zinc-200 bg-white p-1 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                                    <div class="max-h-60 overflow-y-auto no-scrollbar flex flex-col gap-0.5">
                                        {#each availableTags.filter(t => !['Genre', 'Format', 'Key', 'Character'].includes(t.category)) as tag}
                                            <label class="flex items-center gap-2 rounded-md px-2 py-1.5 text-xs hover:bg-zinc-100 dark:hover:bg-zinc-800 cursor-pointer"><input type="checkbox" checked={isTagActive(tag.category, tag.value)} onchange={() => toggleFilterTag(tag.category, tag.value)} class="rounded border-zinc-300 dark:border-zinc-700 accent-zinc-900 dark:accent-zinc-100 cursor-pointer"> {tag.value}</label>
                                        {/each}
                                    </div>
                                </div>
                            </div>
                            <div class="relative">
                                <button onclick={(e) => toggleDropdown('genre', e)} class="flex h-8 items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 text-xs font-semibold transition-colors cursor-pointer {appState.filters.genres.length > 0 ? 'border-zinc-900 dark:border-zinc-100 text-zinc-900 dark:text-zinc-100' : 'text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800'}">
                                    Genre {#if appState.filters.genres.length > 0}<span class="flex h-4 w-4 items-center justify-center rounded-full bg-zinc-900 text-[9px] text-white dark:bg-zinc-100 dark:text-zinc-900">{appState.filters.genres.length}</span>{/if}<ChevronDown size={14} class="opacity-50" />
                                </button>
                                <div onclick={(e) => e.stopPropagation()} class="absolute left-0 top-full mt-1 {openDropdown === 'genre' ? 'flex' : 'hidden'} w-48 flex-col rounded-lg border border-zinc-200 bg-white p-1 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                                    <div class="max-h-60 overflow-y-auto no-scrollbar flex flex-col gap-0.5">
                                        {#each availableTags.filter(t => t.category === 'Genre') as tag}
                                            <label class="flex items-center gap-2 rounded-md px-2 py-1.5 text-xs hover:bg-zinc-100 dark:hover:bg-zinc-800 cursor-pointer"><input type="checkbox" checked={isTagActive('Genre', tag.value)} onchange={() => toggleFilterTag('Genre', tag.value)} class="rounded border-zinc-300 dark:border-zinc-700 accent-zinc-900 dark:accent-zinc-100 cursor-pointer"> {tag.value}</label>
                                        {/each}
                                    </div>
                                </div>
                            </div>
                            <div class="relative">
                                <button onclick={(e) => toggleDropdown('key', e)} class="flex h-8 items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 text-xs font-semibold transition-colors cursor-pointer {appState.filters.keys.length > 0 ? 'border-zinc-900 dark:border-zinc-100 text-zinc-900 dark:text-zinc-100' : 'text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800'}">
                                    Key {#if appState.filters.keys.length > 0}<span class="flex h-4 w-4 items-center justify-center rounded-full bg-zinc-900 text-[9px] text-white dark:bg-zinc-100 dark:text-zinc-900">{appState.filters.keys.length}</span>{/if}<ChevronDown size={14} class="opacity-50" />
                                </button>
                                <div onclick={(e) => e.stopPropagation()} class="absolute left-0 top-full mt-1 {openDropdown === 'key' ? 'flex' : 'hidden'} w-64 flex-col gap-3 rounded-lg border border-zinc-200 bg-white p-3 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                                    <div class="flex rounded-lg bg-zinc-100 p-1 dark:bg-zinc-800/50 border border-zinc-200 dark:border-zinc-700/50 w-full">
                                        <button onclick={() => switchKeyMode('min')} class="flex-1 py-1.5 text-xs font-semibold rounded-md transition-all cursor-pointer {currentKeyMode === 'min' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">Minor</button>
                                        <button onclick={() => switchKeyMode('maj')} class="flex-1 py-1.5 text-xs font-semibold rounded-md transition-all cursor-pointer {currentKeyMode === 'maj' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">Major</button>
                                    </div>
                                    <div class="relative flex w-full h-24 rounded border border-zinc-300 dark:border-zinc-700 overflow-hidden select-none">
                                        {#each whiteKeys as note} <button onclick={() => togglePianoKey(note)} class="flex-1 flex items-end justify-center pb-2 text-[10px] font-bold border-r border-zinc-200 dark:border-zinc-700 last:border-0 transition-colors cursor-pointer {isPianoKeyActive(note) ? 'bg-zinc-200 dark:bg-zinc-600 text-zinc-900 dark:text-white shadow-inner' : 'bg-white dark:bg-zinc-800 text-zinc-800 dark:text-zinc-300 hover:bg-zinc-50 dark:hover:bg-zinc-700'}">{note}</button> {/each}
                                        {#each blackKeys as bk} <button onclick={() => togglePianoKey(bk.note)} style="left: {bk.left}; transform: translateX(-50%);" class="absolute top-0 w-[9%] h-14 rounded-b flex items-end justify-center pb-1.5 text-[8px] font-bold transition-colors cursor-pointer z-10 {isPianoKeyActive(bk.note) ? 'bg-zinc-500 text-white shadow-inner' : 'bg-zinc-900 text-zinc-300 hover:bg-zinc-800 dark:bg-black dark:hover:bg-zinc-900'}">{bk.note}</button> {/each}
                                    </div>
                                </div>
                            </div>
                            <div class="relative">
                                <button onclick={(e) => toggleDropdown('bpm', e)} class="flex h-8 items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 text-xs font-semibold transition-colors cursor-pointer {(appState.filters.bpm.exact || appState.filters.bpm.min || appState.filters.bpm.max) ? 'border-zinc-900 dark:border-zinc-100 text-zinc-900 dark:text-zinc-100' : 'text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800'}">
                                    BPM {#if appState.filters.bpm.exact || appState.filters.bpm.min || appState.filters.bpm.max}<span class="flex h-4 w-4 items-center justify-center rounded-full bg-zinc-900 text-[9px] text-white dark:bg-zinc-100 dark:text-zinc-900">!</span>{/if}<ChevronDown size={14} class="opacity-50" />
                                </button>
                                <div onclick={(e) => e.stopPropagation()} class="absolute left-0 top-full mt-1 {openDropdown === 'bpm' ? 'flex' : 'hidden'} w-56 flex-col gap-3 rounded-lg border border-zinc-200 bg-white p-3 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                                    <div class="flex items-center justify-between border-b border-zinc-100 pb-2 dark:border-zinc-800">
                                        <span class="text-xs font-semibold">Mode</span><label class="flex items-center gap-2 text-[10px] uppercase font-bold tracking-wider cursor-pointer"><input type="checkbox" bind:checked={appState.filters.bpm.isRange} class="rounded border-zinc-300 accent-zinc-900"> Range</label>
                                    </div>
                                    {#if appState.filters.bpm.isRange}
                                        <div class="flex items-center gap-2"><input type="number" bind:value={appState.filters.bpm.min} placeholder="Min" class="w-full rounded-md border border-zinc-200 bg-zinc-50 px-2 py-1 text-xs outline-none focus:border-zinc-900 dark:border-zinc-700 dark:bg-zinc-900 dark:focus:border-zinc-100"><span class="text-xs text-zinc-500">-</span><input type="number" bind:value={appState.filters.bpm.max} placeholder="Max" class="w-full rounded-md border border-zinc-200 bg-zinc-50 px-2 py-1 text-xs outline-none focus:border-zinc-900 dark:border-zinc-700 dark:bg-zinc-900 dark:focus:border-zinc-100"></div>
                                    {:else}
                                        <input type="number" bind:value={appState.filters.bpm.exact} placeholder="Exact BPM (e.g. 120)" class="w-full rounded-md border border-zinc-200 bg-zinc-50 px-2 py-1 text-xs outline-none focus:border-zinc-900 dark:border-zinc-700 dark:bg-zinc-900 dark:focus:border-zinc-100">
                                    {/if}
                                    <div class="flex gap-2 mt-1">
                                        <button onclick={() => { appState.filters.bpm.exact = null; appState.filters.bpm.min = null; appState.filters.bpm.max = null; currentPage = 1; loadSamples(); openDropdown = null; }} class="w-1/3 rounded border border-zinc-200 bg-white px-2 py-1.5 text-xs text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700 cursor-pointer">Clear</button>
                                        <button onclick={() => { openDropdown = null; currentPage = 1; loadSamples(); }} class="w-2/3 rounded bg-zinc-900 px-2 py-1.5 text-xs text-white hover:bg-zinc-800 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-white cursor-pointer">Apply</button>
                                    </div>
                                </div>
                            </div>

                            <div class="relative">
                                <button onclick={(e) => toggleDropdown('format', e)} class="flex h-8 items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 text-xs font-semibold transition-colors cursor-pointer {appState.filters.formats.length > 0 ? 'border-zinc-900 dark:border-zinc-100 text-zinc-900 dark:text-zinc-100' : 'text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800'}">
                                    Format {#if appState.filters.formats.length > 0}<span class="flex h-4 w-4 items-center justify-center rounded-full bg-zinc-900 text-[9px] text-white dark:bg-zinc-100 dark:text-zinc-900">{appState.filters.formats.length}</span>{/if}<ChevronDown size={14} class="opacity-50" />
                                </button>
                                <div onclick={(e) => e.stopPropagation()} class="absolute left-0 top-full mt-1 {openDropdown === 'format' ? 'flex' : 'hidden'} w-40 flex-col rounded-lg border border-zinc-200 bg-white p-1 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                                    <div class="max-h-60 overflow-y-auto no-scrollbar flex flex-col gap-0.5">
                                        {#each availableTags.filter(t => t.category === 'Format') as tag}
                                            <label class="flex items-center gap-2 rounded-md px-2 py-1.5 text-xs hover:bg-zinc-100 dark:hover:bg-zinc-800 cursor-pointer"><input type="checkbox" checked={isTagActive('Format', tag.value)} onchange={() => toggleFilterTag('Format', tag.value)} class="rounded border-zinc-300 dark:border-zinc-700 accent-zinc-900 dark:accent-zinc-100 cursor-pointer"> {tag.value}</label>
                                        {/each}
                                    </div>
                                </div>
                            </div>

                            {#if appState.filters.instruments.length > 0 || appState.filters.genres.length > 0 || appState.filters.formats.length > 0 || appState.filters.keys.length > 0 || appState.filters.bpm.exact || appState.filters.bpm.min || appState.filters.bpm.max}
                                <button onclick={clearAllFilters} class="ml-2 flex h-8 items-center text-xs font-semibold text-red-600 hover:text-red-700 dark:text-red-400 dark:hover:text-red-300 transition-colors cursor-pointer">Clear all</button>
                            {/if}

                            <div class="ml-auto flex items-center gap-2">
                                <span class="text-[10px] font-bold uppercase tracking-wider text-zinc-400">Match:</span>
                                <div class="flex h-8 rounded-full border border-zinc-200 bg-zinc-50 p-[3px] dark:border-zinc-700/50 dark:bg-zinc-900">
                                    <button onclick={() => { appState.filters.tagMatchMode = 'OR'; currentPage = 1; loadSamples(); }} class="px-3 text-[10px] font-bold uppercase tracking-wider rounded-full transition-colors cursor-pointer {appState.filters.tagMatchMode === 'OR' ? 'bg-emerald-500 text-white shadow-sm' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">Either</button>
                                    <button onclick={() => { appState.filters.tagMatchMode = 'AND'; currentPage = 1; loadSamples(); }} class="px-3 text-[10px] font-bold uppercase tracking-wider rounded-full transition-colors cursor-pointer {appState.filters.tagMatchMode === 'AND' ? 'bg-emerald-500 text-white shadow-sm' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">Both</button>
                                </div>
                            </div>
                        </div>

                        <div class="relative w-full">
                            <div class="flex w-full flex-wrap content-start items-start gap-2 pr-10 transition-all {isTagsExpanded ? 'h-auto pb-1' : 'h-6 overflow-hidden'}">
                                {#each sortedAvailableTags as tag (tag.category + tag.value)}
                                    <button onclick={() => toggleFilterTag(tag.category, tag.value)} class="shrink-0 flex items-center h-6 rounded-full border px-3 text-[11px] font-semibold cursor-pointer transition-colors {isTagActive(tag.category, tag.value) ? 'border-zinc-900 bg-zinc-900 text-white dark:border-zinc-100 dark:bg-zinc-100 dark:text-zinc-900' : 'border-zinc-200 bg-zinc-50 text-zinc-600 hover:border-zinc-300 hover:bg-zinc-100 dark:border-zinc-800 dark:bg-[#18181b] dark:text-zinc-400 dark:hover:bg-zinc-800'}">{tag.value} {#if isTagActive(tag.category, tag.value)}<span class="ml-1.5 opacity-50 font-normal hover:opacity-100">✕</span>{/if}</button>
                                {/each}
                            </div>
                            {#if sortedAvailableTags.length > 0}
                                <div class="absolute right-0 top-0 h-6 bg-gradient-to-l from-white via-white to-transparent pl-8 pr-1 dark:from-[#18181b] dark:via-[#18181b]">
                                    <button onclick={() => isTagsExpanded = !isTagsExpanded} class="flex h-full items-center justify-center rounded px-1.5 text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer">{#if isTagsExpanded}<span class="text-sm font-bold leading-none mt-[1px]">✕</span>{:else}<span class="text-sm font-bold leading-none tracking-widest -mt-2">...</span>{/if}</button>
                                </div>
                            {/if}
                        </div>
                    </div>

                    <div class="mb-4 flex items-center justify-between text-sm text-zinc-500 relative">
                        <span>{#if isLoading}Loading...{:else}{totalItems} results{/if}</span>
                        <div class="flex items-center gap-3">
                            <span class="text-xs font-medium">Sort by:</span>
                            <div class="flex items-center rounded-md border border-zinc-200 bg-white p-0.5 shadow-sm dark:border-zinc-700 dark:bg-zinc-900">
                                <div class="relative">
                                    <button onclick={(e) => { e.stopPropagation(); isSortDropdownOpen = !isSortDropdownOpen; openDropdown = null; }} class="flex h-7 items-center gap-2 rounded px-2 text-xs font-semibold text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800 transition-colors cursor-pointer">{sortField === 'name' ? 'Alphabetical' : sortField === 'type' ? 'Instrument Type' : sortField === 'pack' ? 'Sample Pack' : 'Randomize'} <ChevronDown size={14} class="opacity-50" /></button>
                                    {#if isSortDropdownOpen}
                                        <div onclick={(e) => e.stopPropagation()} class="absolute right-0 top-full mt-1 w-40 flex-col rounded-lg border border-zinc-200 bg-white p-1 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                                            <button onclick={() => { sortField = 'name'; isSortDropdownOpen = false; currentPage = 1; loadSamples(); }} class="w-full text-left rounded-md px-2 py-1.5 text-xs hover:bg-zinc-100 dark:hover:bg-zinc-800 cursor-pointer {sortField === 'name' ? 'font-bold text-zinc-900 dark:text-white' : ''}">Alphabetical</button>
                                            <button onclick={() => { sortField = 'type'; isSortDropdownOpen = false; currentPage = 1; loadSamples(); }} class="w-full text-left rounded-md px-2 py-1.5 text-xs hover:bg-zinc-100 dark:hover:bg-zinc-800 cursor-pointer {sortField === 'type' ? 'font-bold text-zinc-900 dark:text-white' : ''}">Instrument Type</button>
                                            <button onclick={() => { sortField = 'pack'; isSortDropdownOpen = false; currentPage = 1; loadSamples(); }} class="w-full text-left rounded-md px-2 py-1.5 text-xs hover:bg-zinc-100 dark:hover:bg-zinc-800 cursor-pointer {sortField === 'pack' ? 'font-bold text-zinc-900 dark:text-white' : ''}">Sample Pack</button>
                                        </div>
                                    {/if}
                                </div>
                                <div class="h-4 w-px bg-zinc-200 dark:bg-zinc-700 mx-0.5"></div>
                                <button onclick={() => { sortOrder = sortOrder === 'asc' ? 'desc' : 'asc'; if(sortField !== 'random') { currentPage = 1; loadSamples(); } }} disabled={sortField === 'random'} class="flex h-7 w-7 items-center justify-center rounded text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 disabled:opacity-30 disabled:hover:bg-transparent dark:hover:bg-zinc-800 dark:hover:text-zinc-100 transition-colors cursor-pointer" title="Reverse Order"><ArrowDownUp size={14} class={sortOrder === 'desc' ? 'rotate-180 transition-transform' : 'transition-transform'} /></button>
                                <button onclick={() => { sortField = 'random'; currentPage = 1; loadSamples(); }} class="flex h-7 w-7 items-center justify-center rounded text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-zinc-100 transition-colors cursor-pointer {sortField === 'random' ? 'bg-zinc-200 text-zinc-900 dark:bg-zinc-700 dark:text-zinc-100 shadow-inner' : ''}" title="Shuffle"><Shuffle size={14} /></button>
                            </div>
                        </div>
                    </div>

                    <div class="grid grid-cols-[20px_40px_32px_minmax(150px,2fr)_minmax(120px,1.5fr)_50px_40px_40px_32px_32px] gap-4 border-y border-zinc-200 py-2 text-[11px] font-semibold uppercase tracking-wider text-zinc-500 dark:border-zinc-800 items-center px-2">
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
                            <div class="flex flex-col items-center justify-center h-64 text-zinc-500"><span class="text-sm font-medium">No samples found matching these filters.</span><button onclick={clearAllFilters} class="mt-4 text-xs font-semibold text-zinc-900 dark:text-zinc-100 underline cursor-pointer">Clear all filters</button></div>
                        {:else}
                            <div class="divide-y divide-zinc-100 dark:divide-zinc-800/50 mb-8 transition-opacity duration-200 {isLoading ? 'opacity-40 pointer-events-none' : 'opacity-100'}">
                                {#each samples as sample}
                                    <div id="sample-{sample.id}" class="group grid grid-cols-[20px_40px_32px_minmax(150px,2fr)_minmax(120px,1.5fr)_50px_40px_40px_32px_32px] items-center gap-4 py-2 rounded-md -mx-2 px-2 {selectedId === sample.id ? 'bg-zinc-100 dark:bg-zinc-800/60' : 'hover:bg-zinc-50 dark:hover:bg-zinc-800/20'}">
                                        <div class="flex justify-center"><input type="checkbox" checked={appState.selectedSampleIds.includes(sample.id)} onchange={(e) => toggleSampleSelection(sample.id, e.currentTarget.checked)} class="h-4 w-4 rounded border-zinc-300 bg-zinc-100 cursor-pointer accent-zinc-900 dark:accent-zinc-100"></div>
                                        <div class="h-10 w-10 flex items-center justify-center rounded-md bg-zinc-200/50 text-zinc-400 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700/50"><ImageIcon size={20} /></div>
                                        <div class="flex justify-center"><button onclick={() => handlePlayRequest(sample)} class="flex h-8 w-8 items-center justify-center rounded-full bg-zinc-900 text-zinc-100 hover:scale-105 dark:bg-zinc-100 dark:text-zinc-900 transition-transform cursor-pointer shadow-sm">{#if playingId === sample.id} <Pause size={14} /> {:else} <Play size={14} class="ml-0.5" /> {/if}</button></div>

                                        <div class="flex flex-col min-w-0 pr-4 cursor-pointer" role="button" tabindex="0" onclick={() => { selectedId = sample.id; }} onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') selectedId = sample.id; }}>
                                            <span class="truncate text-sm font-semibold cursor-pointer hover:underline" title={sample.original_path}>{sample.filename}</span>
                                            <div class="flex flex-wrap gap-1.5 mt-1 h-4 overflow-hidden">
                                                {#each parseTags(sample.tags) as tag}
                                                    <span class="rounded px-1.5 py-[1px] text-[9px] font-bold uppercase tracking-wider {tag.category === 'Format' ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400' : tag.category === 'Drums' || tag.category === 'Percussion' ? 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400' : tag.category === 'Genre' ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400' : 'bg-zinc-200/60 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400'}">{tag.value}</span>
                                                {/each}
                                                {#if parseTags(sample.tags).length === 0} <span class="rounded bg-zinc-200/60 px-1.5 py-[1px] text-[9px] font-bold uppercase tracking-wider text-zinc-400 dark:bg-zinc-800">AUDIO</span> {/if}
                                            </div>
                                        </div>

                                        <div class="flex items-center gap-[2px] h-8 overflow-hidden opacity-60 group-hover:opacity-100 transition-opacity">
                                            {#each parseWaveform(sample.waveform_data) as barHeight, i} <div class="w-[3px] rounded-full {playingId === sample.id && (i / 40) <= playbackProgress ? 'bg-emerald-500' : 'bg-zinc-300 dark:bg-zinc-700'}" style="height: {barHeight}%;"></div> {/each}
                                        </div>

                                        <div class="text-right text-xs font-medium text-zinc-500 tabular-nums">{formatDuration(sample.duration_ms)}</div>
                                        <div class="text-center text-xs font-semibold text-zinc-700 dark:text-zinc-300">{sample.key_signature || "--"}</div>
                                        <div class="text-center text-xs font-semibold text-zinc-700 dark:text-zinc-300">{sample.bpm ? Math.round(sample.bpm) : "--"}</div>
                                        <div class="flex justify-center"><button onclick={(e) => toggleLike(sample, e)} class="transition-colors cursor-pointer group-hover:opacity-100 {selectedId === sample.id || sample.is_liked ? 'opacity-100' : 'opacity-0'} {sample.is_liked ? 'text-red-500 hover:text-red-600' : 'text-zinc-400 hover:text-red-500'}"><Heart size={16} class={sample.is_liked ? 'fill-red-500' : ''} /></button></div>
                                        <div class="relative flex justify-center">
                                            <button onclick={(e) => { e.stopPropagation(); openContextMenuId = openContextMenuId === sample.id ? null : sample.id; }} class="text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer group-hover:opacity-100 {selectedId === sample.id || openContextMenuId === sample.id ? 'opacity-100' : 'opacity-0'}"><EllipsisVertical size={16} /></button>
                                            {#if openContextMenuId === sample.id}
                                                <div onclick={(e) => e.stopPropagation()} class="absolute right-full top-0 mr-2 w-40 flex-col rounded-lg border border-zinc-200 bg-white p-1 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                                                    <button onclick={() => openEditModal(sample)} class="w-full text-left rounded-md px-3 py-2 text-xs font-medium text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800 cursor-pointer transition-colors">Edit Metadata</button>
                                                    <div class="my-0.5 border-t border-zinc-200 dark:border-zinc-800/50"></div>
                                                    <button onclick={() => { invoke('reveal_in_finder', { path: sample.original_path }); openContextMenuId = null; }} class="w-full text-left rounded-md px-3 py-2 text-xs font-medium text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800 cursor-pointer transition-colors">Reveal in Finder</button>
                                                </div>
                                            {/if}
                                        </div>
                                    </div>
                                {/each}
                            </div>

                            {#if totalPages > 1}
                                <div class="flex items-center justify-center pb-8 pt-4">
                                    <div class="flex items-center gap-1">
                                        <button onclick={prevPage} disabled={currentPage === 1} class="flex items-center justify-center h-8 w-8 rounded text-zinc-600 hover:bg-zinc-100 disabled:opacity-30 disabled:hover:bg-transparent dark:text-zinc-400 dark:hover:bg-zinc-800 transition-colors cursor-pointer mr-2"><ChevronLeft size={18} /></button>
                                        {#each visiblePages as pageNum} <button onclick={() => goToPage(pageNum)} class="flex items-center justify-center h-8 w-8 rounded text-sm font-medium transition-colors cursor-pointer {pageNum === currentPage ? 'bg-zinc-900 text-white dark:bg-zinc-100 dark:text-zinc-900' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800'}">{pageNum}</button> {/each}
                                        <button onclick={nextPage} disabled={currentPage === totalPages} class="flex items-center justify-center h-8 w-8 rounded text-zinc-600 hover:bg-zinc-100 disabled:opacity-30 disabled:hover:bg-transparent dark:text-zinc-400 dark:hover:bg-zinc-800 transition-colors cursor-pointer ml-2"><ChevronRight size={18} /></button>
                                    </div>
                                </div>
                            {/if}
                        {/if}
                    </div>
                </div>
            {/if}
        </div>

        <div class="w-72 shrink-0 bg-white/95 backdrop-blur-xl border-l border-zinc-200 dark:border-zinc-800/60 dark:bg-[#18181b]/95 shadow-2xl transition-all duration-300 flex flex-col z-40 {appState.selectedSampleIds.length > 0 ? 'mr-0' : '-mr-72'}">
            <div class="flex items-center justify-between px-5 py-4 border-b border-zinc-200 dark:border-zinc-800/60">
                <span class="font-bold text-sm">{appState.selectedSampleIds.length} Items Selected</span>
                <button onclick={() => appState.selectedSampleIds = []} class="text-zinc-400 hover:text-zinc-900 dark:hover:text-white cursor-pointer"><X size={16} /></button>
            </div>
            <div class="flex-1 overflow-y-auto p-3 space-y-1">
                <span class="px-2 text-[10px] font-bold uppercase tracking-wider text-zinc-400">Add to...</span>
                <div class="flex w-full items-center justify-between rounded-md px-2 py-1.5 transition-colors hover:bg-zinc-50 dark:hover:bg-zinc-800/30">
                    <div class="flex items-center gap-3 text-sm font-medium text-zinc-600 dark:text-zinc-400"><Heart size={16} /> Liked Folder</div>
                    <button onclick={handleBulkLike} class="rounded border border-zinc-200 bg-white px-3 py-1 text-xs font-semibold text-zinc-700 shadow-sm transition-colors hover:bg-zinc-100 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700 cursor-pointer">Add</button>
                </div>
                <div class="my-2 border-t border-zinc-200 dark:border-zinc-800/50"></div>
                {#each appState.collections as collection}
                    <div class="flex w-full items-center justify-between rounded-md px-2 py-1.5 transition-colors hover:bg-zinc-50 dark:hover:bg-zinc-800/30">
                        <div class="flex items-center gap-3 text-sm font-medium text-zinc-600 dark:text-zinc-400"><Folder size={16} /> {collection.name}</div>
                        <button onclick={() => handleBulkAddToCollection(collection.id)} class="rounded border border-zinc-200 bg-white px-3 py-1 text-xs font-semibold text-zinc-700 shadow-sm transition-colors hover:bg-zinc-100 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700 cursor-pointer">Add</button>
                    </div>
                {/each}
                <div class="my-2 border-t border-zinc-200 dark:border-zinc-800/50"></div>
                <button onclick={() => appState.isCreateCollectionModalOpen = true} class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/50 dark:hover:text-zinc-100 transition-colors cursor-pointer"><Plus size={16} /> New Collection</button>
            </div>
        </div>

    </div>
{:else if appState.currentView === 'projects'}
    <div class="flex h-full items-center justify-center text-zinc-500"><h2 class="text-2xl font-bold">Musik Projekte (Coming Soon)</h2></div>
{:else if appState.currentView === 'editor'}
    <div class="flex h-full items-center justify-center text-zinc-500"><h2 class="text-2xl font-bold">Pack Editor & Renamer (Coming Soon)</h2></div>
{:else if appState.currentView === 'settings'}
    <div class="flex h-full w-full flex-col overflow-y-auto px-10 py-8">
        <h1 class="mb-8 text-3xl font-bold tracking-tight">Preferences</h1>
        <div class="max-w-3xl">
            {#if appState.activeSettingsTab === 'general'}
                <div class="space-y-4">
                    <h3 class="text-xs font-bold uppercase tracking-wider text-zinc-400 border-b border-zinc-200 dark:border-zinc-800/50 pb-2 mb-6">Appearance</h3>
                    <label class="text-sm font-medium text-zinc-900 dark:text-zinc-100 block">Theme Preference</label>
                    <div class="flex rounded-lg bg-zinc-100 p-1 dark:bg-zinc-800/50 border border-zinc-200 dark:border-zinc-700/50 w-fit">
                        <button onclick={() => setThemePref('light')} class="px-6 py-2 text-xs font-semibold rounded-md transition-all cursor-pointer {appState.themePreference === 'light' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">Light</button>
                        <button onclick={() => setThemePref('dark')} class="px-6 py-2 text-xs font-semibold rounded-md transition-all cursor-pointer {appState.themePreference === 'dark' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">Dark</button>
                        <button onclick={() => setThemePref('system')} class="px-6 py-2 text-xs font-semibold rounded-md transition-all cursor-pointer {appState.themePreference === 'system' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">System</button>
                    </div>
                    <p class="text-xs text-zinc-500 pt-2">"System" automatically matches your Mac's appearance settings.</p>
                </div>
            {:else if appState.activeSettingsTab === 'library'}
                <div>
                    <h3 class="text-xs font-bold uppercase tracking-wider text-zinc-400 border-b border-zinc-200 dark:border-zinc-800/50 pb-2 mb-6">Connected Folders</h3>
                    {#if isSettingsLoading}
                        <div class="flex h-20 items-center justify-center text-sm text-zinc-500 animate-pulse">Loading library data...</div>
                    {:else if connectedFolders.length === 0}
                        <div class="flex h-20 items-center justify-center rounded-md border border-dashed border-zinc-200 text-sm text-zinc-500 dark:border-zinc-800">No folders connected yet.</div>
                    {:else}
                        <div class="space-y-2">
                            {#each connectedFolders as folder}
                                <div class="flex items-center justify-between rounded-md border border-zinc-200 bg-white p-3 shadow-sm dark:border-zinc-800 dark:bg-zinc-900/50">
                                    <div class="flex flex-col overflow-hidden pr-4"><span class="truncate text-sm font-medium text-zinc-700 dark:text-zinc-300" title={folder}>{folder}</span></div>
                                    <button onclick={() => handleRemoveFolder(folder)} disabled={isSettingsLoading} class="shrink-0 rounded-md border border-red-200 bg-red-50 px-3 py-1.5 text-xs font-medium text-red-700 hover:bg-red-100 transition-colors dark:border-red-900/30 dark:bg-red-900/10 dark:text-red-400 dark:hover:bg-red-900/20 cursor-pointer disabled:opacity-50">Un-link</button>
                                </div>
                            {/each}
                        </div>
                    {/if}
                    <p class="mt-4 text-xs text-zinc-500">Un-linking a folder removes all its indexed samples from this application. It does not delete the actual files from your computer.</p>
                    <div class="mt-12 border-t border-zinc-200 pt-8 dark:border-zinc-800/50">
                        <h3 class="mb-4 text-xs font-bold uppercase tracking-wider text-red-500">Danger Zone</h3>
                        <div class="flex items-center justify-between rounded-md border border-red-200 bg-red-50 p-4 dark:border-red-900/30 dark:bg-red-900/10">
                            <div class="flex flex-col pr-4">
                                <span class="text-sm font-bold text-red-700 dark:text-red-400">Clear Entire Library</span>
                                <span class="text-xs text-red-600/80 dark:text-red-400/80 mt-1">This will instantly wipe all indexed samples and collections from your database. Your actual audio files on the hard drive remain untouched.</span>
                            </div>
                            <button onclick={handleClearDatabase} disabled={isSettingsLoading || isScanning} class="shrink-0 rounded-md bg-red-600 px-4 py-2 text-xs font-bold text-white transition-colors hover:bg-red-700 disabled:opacity-50 cursor-pointer shadow-sm">
                                {#if isClearing} <RefreshCw size={14} class="animate-spin inline mr-1" /> Clearing... {:else} Clear Database {/if}
                            </button>
                        </div>
                    </div>
                </div>
            {:else if appState.activeSettingsTab === 'audio'}
                <div>
                    <h3 class="text-xs font-bold uppercase tracking-wider text-zinc-400 border-b border-zinc-200 dark:border-zinc-800/50 pb-2 mb-6">Audio Engine</h3>
                    <div class="flex h-32 items-center justify-center rounded-md border border-dashed border-zinc-200 text-sm text-zinc-500 dark:border-zinc-800">Audio Device Routing Options (Coming Soon)</div>
                </div>
            {/if}
        </div>
    </div>
{/if}

{#if editingSample}
    <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm">
        <div class="w-full max-w-md rounded-xl border border-zinc-200 bg-white shadow-2xl dark:border-zinc-800 dark:bg-[#18181b] p-6">
            <h2 class="text-lg font-bold text-zinc-900 dark:text-zinc-100 mb-6">Edit Properties</h2>
            <div class="space-y-4">
                <div>
                    <label class="block text-[10px] font-bold text-zinc-500 uppercase tracking-wider mb-1.5">Filename</label>
                    <input type="text" bind:value={editForm.filename} class="w-full rounded-md border border-zinc-300 bg-white px-3 py-2 text-sm focus:border-zinc-900 focus:outline-none dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100 dark:focus:border-zinc-100 transition-colors" />
                </div>
                <div class="flex gap-4">
                    <div class="flex-1">
                        <label class="block text-[10px] font-bold text-zinc-500 uppercase tracking-wider mb-1.5">BPM</label>
                        <input type="number" bind:value={editForm.bpm} placeholder="e.g. 120" class="w-full rounded-md border border-zinc-300 bg-white px-3 py-2 text-sm focus:border-zinc-900 focus:outline-none dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100 dark:focus:border-zinc-100 transition-colors" />
                    </div>
                    <div class="flex-1">
                        <label class="block text-[10px] font-bold text-zinc-500 uppercase tracking-wider mb-1.5">Key</label>
                        <input type="text" bind:value={editForm.key_signature} placeholder="e.g. F# min" class="w-full rounded-md border border-zinc-300 bg-white px-3 py-2 text-sm focus:border-zinc-900 focus:outline-none dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100 dark:focus:border-zinc-100 transition-colors" />
                    </div>
                </div>
                <div>
                    <label class="block text-[10px] font-bold text-zinc-500 uppercase tracking-wider mb-1.5">Tags</label>
                    <div class="flex flex-wrap gap-2 items-center min-h-[42px] rounded-md border border-zinc-300 bg-white px-3 py-2 dark:border-zinc-700 dark:bg-zinc-900 transition-colors">
                        {#each editForm.tags as tag, i}
                            <div class="group relative flex items-center justify-center h-6 rounded-full border border-zinc-200 bg-zinc-50 px-2.5 text-[11px] font-semibold text-zinc-600 transition-all hover:pr-6 dark:border-zinc-800 dark:bg-[#18181b] dark:text-zinc-300 cursor-default overflow-hidden shadow-sm">
                                <span>{tag.value}</span>
                                <button onclick={() => removeTagFromEditor(i)} class="absolute right-1 opacity-0 group-hover:opacity-100 flex h-4 w-4 items-center justify-center rounded-full bg-zinc-200 text-zinc-600 hover:bg-red-500 hover:text-white dark:bg-zinc-700 dark:text-zinc-300 dark:hover:bg-red-600 transition-all cursor-pointer"><X size={10} /></button>
                            </div>
                        {/each}
                        <div class="relative tag-dropdown-container">
                            <button onclick={(e) => { e.stopPropagation(); isTagDropdownOpen = !isTagDropdownOpen; }} class="flex h-6 w-6 items-center justify-center rounded-full border border-dashed border-zinc-300 text-zinc-400 hover:border-zinc-500 hover:text-zinc-600 dark:border-zinc-600 dark:hover:border-zinc-400 dark:hover:text-zinc-300 transition-colors cursor-pointer"><Plus size={14} /></button>
                            {#if isTagDropdownOpen}
                                <div onclick={(e) => e.stopPropagation()} class="absolute left-0 top-full mt-2 w-72 flex-col rounded-lg border border-zinc-200 bg-white p-2 shadow-2xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                                    <input type="text" bind:value={tagSearchQuery} placeholder="Search or create tag..." class="w-full rounded-md border border-zinc-200 bg-zinc-50 px-2 py-1.5 text-xs focus:border-emerald-500 focus:outline-none dark:border-zinc-700 dark:bg-zinc-900 dark:text-white transition-colors mb-2" />
                                    {#if tagSearchQuery.trim() !== '' && !allAvailableTags.some(t => t.value.toLowerCase() === tagSearchQuery.toLowerCase())}
                                        <button onclick={createNewTag} class="w-full mb-2 flex items-center justify-center gap-1.5 rounded-md bg-emerald-50 text-emerald-600 px-2 py-1.5 text-xs font-bold hover:bg-emerald-100 dark:bg-emerald-900/20 dark:text-emerald-400 dark:hover:bg-emerald-900/40 transition-colors cursor-pointer"><Plus size={12} /> Create global Tag "{tagSearchQuery}"</button>
                                    {/if}
                                    <div class="max-h-60 overflow-y-auto no-scrollbar flex flex-col gap-0.5 border-t border-zinc-100 dark:border-zinc-800/50 pt-2">
                                        {#each filteredTagsForEditor as tag}
                                            <button
                                                    onclick={() => addTagToEditor(tag.category, tag.value)}
                                                    class="group/item w-full text-left flex items-center justify-between rounded-md px-2 py-1.5 text-xs font-medium text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800 cursor-pointer transition-colors"
                                            >
                                                <span class="truncate pr-2">{tag.value}</span>

                                                <div class="flex items-center gap-2 shrink-0 relative pr-5">
                                                    <span class="text-[8px] uppercase text-zinc-400 font-bold tracking-wider">{tag.category}</span>

                                                    {#if tag.category === 'User'}
                                                        <div
                                                                role="button"
                                                                tabindex="0"
                                                                onclick={(e) => handleDeleteUserTag(tag.value, e)}
                                                                onkeydown={(e) => { if (e.key === 'Enter') handleDeleteUserTag(tag.value, e); }}
                                                                class="absolute right-0 opacity-0 group-hover/item:opacity-100 p-0.5 rounded-md hover:bg-red-100 hover:text-red-600 dark:hover:bg-red-900/30 transition-all"
                                                        >
                                                            <Trash2 size={10} />
                                                        </div>
                                                    {/if}
                                                </div>
                                            </button>
                                        {/each}
                                    </div>
                                </div>
                            {/if}
                        </div>
                    </div>
                </div>
            </div>
            <div class="mt-8 flex justify-end gap-3">
                <button onclick={() => editingSample = null} class="px-4 py-2 text-sm font-semibold text-zinc-600 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100 transition-colors cursor-pointer">Cancel</button>
                <button onclick={saveMetadata} class="rounded-md bg-zinc-900 px-5 py-2 text-sm font-semibold text-white shadow-sm hover:bg-zinc-800 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-white transition-colors cursor-pointer">Save Changes</button>
            </div>
        </div>
    </div>
{/if}