<script lang="ts">
    import { onMount, onDestroy, tick } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { appDataDir } from '@tauri-apps/api/path';
    import { open, ask } from '@tauri-apps/plugin-dialog';
    import { writeFile, mkdir } from '@tauri-apps/plugin-fs';
    import { EllipsisVertical, Download, Heart, Play, Pause, FolderPlus, RefreshCw, Trash2, Image as ImageIcon, ChevronLeft, ChevronRight, Settings, X, ChevronDown, ArrowDownUp, Shuffle, Folder, Plus, Music2, Repeat, Gauge, Search, Library, ZoomIn, ZoomOut } from 'lucide-svelte';
    import { FastForward, Rewind } from 'lucide-svelte';
    import { appState } from '$lib/store.svelte';
    import { startDrag } from '@crabnebula/tauri-plugin-drag';

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
        cover_path: string | null; // <--- NEU: Der Pfad zum extrahierten Bild
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

    let activeTypeFilter: string | null = $state(null);

    // --- ENTERPRISE EMPTY STATE LOGIC ---
    // Überwacht reaktiv, ob der User aktiv sucht oder filtert
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

    // --- AUDIO STATE (Refactored to Rust Backend) ---
    let playingId: string | null = $state(null);
    let selectedId: string | null = $state(null);

    let playbackProgress: number = $state(0);
    let currentSampleDuration: number = 0;
    // Captured at playback start — actual duration = currentSampleDuration / currentStretchRatio.
    // Keeps animation in sync with the time-stretched audio.
    let currentStretchRatio: number = 1.0;
    let playbackStartTime: number = 0;
    let animationFrameId: number;

    // --- TAGS LOGIC (Richtig sortiert für den Compiler) ---
    const BASE_TAGS = [
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
    ];

    // Initialer State nimmt die Base-Tags, wird später von Rust überschrieben
    let availableTags: Array<{category: string, value: string}> = $state([...BASE_TAGS]);

    // ENTERPRISE FIX: Intelligenter Tag-Merge
    let activeDropdownTags = $derived.by(() => {
        if (appState.filters.tagMatchMode === 'OR') {
            // Im "Either" Modus: Wir vereinen unsere festen Basis-Tags mit den
            // Tags aus der Datenbank und filtern alle doppelten Einträge sauber heraus.
            const combined = [...BASE_TAGS, ...allAvailableTags];
            return combined.filter((tag, index, self) =>
                index === self.findIndex((t) => t.category === tag.category && t.value === tag.value)
            );
        } else {
            // Im "Both" Modus: Wir zeigen streng nur das, was die aktuelle Suche hergibt
            return availableTags;
        }
    });

    let userTags: Array<{category: string, value: string}> = $state([]);

    // --- PROGRESS BAR STATE ---
    type ScanProgressPayload = { total: number; current: number; current_file: string; };
    let scanTotal: number = $state(0);
    let scanCurrent: number = $state(0);
    let scanCurrentFile: string = $state('');
    let scanPercentage = $derived(scanTotal > 0 ? Math.round((scanCurrent / scanTotal) * 100) : 0);

    function updateProgress() {
        if (appState.isPlaying) {
            const elapsed = (performance.now() - playbackStartTime) / 1000;

            if (isSamplerOpen && samplerSample) {
                // ENTERPRISE FIX: Division durch appState.vinylSpeedMode hält die Animation synchron!
                const selectionDurationSec = ((samplerSample.duration_ms * (trimEndPct - trimStartPct)) / 1000 / currentStretchRatio) / appState.vinylSpeedMode;

                if (elapsed >= selectionDurationSec) {
                    if (isLooping && currentPreviewPath) {
                        playbackStartTime = performance.now();
                        appState.playbackProgress = 0;
                        currentStretchRatio = samplerSample ? getStretchRatio(samplerSample) : 1.0;

                        let semitones = 0;
                        if (appState.globalKey && samplerSample.key_signature) {
                            semitones = getSemitoneShift(samplerSample.key_signature, appState.globalKey, appState.globalKeyMode);
                        }

                        // Übergibt den Speed an den Rust-Loop
                        invoke('play_audio', { filePath: currentPreviewPath, semitones, stretchRatio: currentStretchRatio, volume: appState.globalVolume, speed: appState.vinylSpeedMode }).catch(console.error);
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
        isQueueDropdownOpen = false;
        if (isTagDropdownOpen) {
            const target = e.target as HTMLElement;
            if (!target.closest('.tag-dropdown-container')) {
                isTagDropdownOpen = false;
            }
        }
        // In handleGlobalClick einfügen:
        if (isBulkTagDropdownOpen) {
            const target = e.target as HTMLElement;
            if (!target.closest('.bulk-tag-dropdown-container')) {
                isBulkTagDropdownOpen = false;
            }
        }
    };

    onMount(async () => {
        // @ts-ignore
        const AudioContextClass = window.AudioContext || window.webkitAudioContext;

        // appDataDir einmalig auflösen und drag-icons Unterordner anlegen.
        appDataDir().then(async dir => {
            _appDataPath = dir.endsWith('/') || dir.endsWith('\\') ? dir : dir + '/';
            try { await mkdir(_appDataPath + 'drag-icons', { recursive: true }); } catch { /* existiert bereits */ }
        }).catch(e => console.warn('[SampleVault] appDataDir nicht aufgelöst:', e));

        window.addEventListener('keydown', handleKeydown);
        window.addEventListener('click', handleGlobalClick);
        window.addEventListener('trigger-sample-reload', handleSampleReload);
        window.addEventListener('mousemove', handleEditorMouseMove);
        window.addEventListener('mouseup', handleEditorMouseUp);
        window.addEventListener('force-retrigger', handleForceRetrigger);

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
        cancelAnimationFrame(animationFrameId);

        window.removeEventListener('keydown', handleKeydown);
        window.removeEventListener('click', handleGlobalClick);
        window.removeEventListener('trigger-sample-reload', handleSampleReload);
        window.removeEventListener('mousemove', handleEditorMouseMove);
        window.removeEventListener('mouseup', handleEditorMouseUp);
        window.removeEventListener('force-retrigger', handleForceRetrigger);
    });

    $effect(() => {
        invoke('set_audio_volume', { volume: appState.globalVolume });
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

            // ENTERPRISE FIX: Wenn Sampler offen, ignoriere die normale Liste und toggle das Preview!
            if (isSamplerOpen) {
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

    // --- ENTERPRISE SCROLL LOGIC ---
    // Scrollt nur auf der vertikalen Achse (Y) und verhindert das horizontale Verschieben des Layouts
    function scrollToSample(id: string) {
        const el = document.getElementById(`sample-${id}`);
        if (!el || !scrollContainer) return;

        const elRect = el.getBoundingClientRect();
        const containerRect = scrollContainer.getBoundingClientRect();

        // Prüfen, ob das Element oben oder unten aus dem sichtbaren Bereich ragt
        if (elRect.top < containerRect.top) {
            scrollContainer.scrollBy({ top: elRect.top - containerRect.top - 20, behavior: 'smooth' });
        } else if (elRect.bottom > containerRect.bottom) {
            scrollContainer.scrollBy({ top: elRect.bottom - containerRect.bottom + 20, behavior: 'smooth' });
        }
    }

    async function handleKeydown(e: KeyboardEvent) {
        // Ignoriere Tastenkürzel, wenn der Nutzer in einem Suchfeld tippt
        if (document.activeElement?.tagName === 'INPUT') return;

        if (isSamplerOpen) {
            // ArrowLeft ignoriert das Blockieren und wird zum Retrigger!
            if (['ArrowDown', 'ArrowUp', 'ArrowRight'].includes(e.key)) {
                e.preventDefault();
                return;
            }

            // ENTERPRISE FIX: ArrowLeft fungiert als MPC "CUE"-Trigger.
            // Es startet das Preview in 0ms komplett von vorn, egal ob es gerade läuft!
            if (e.key === 'ArrowLeft') {
                e.preventDefault();
                playSlicePreview(true);
                return;
            }

            if (e.key === ' ') {
                e.preventDefault();
                playSlicePreview();
                return;
            }
            if (e.key === 'Escape') {
                e.preventDefault();
                closeSampler();
                return;
            }
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
                            // ENTERPRISE FIX: Auto-Play Prüfung auch beim Seitenwechsel
                            if (appState.autoPlayEnabled !== false) {
                                await handlePlayRequest(samples[0]);
                            } else {
                                selectedId = samples[0].id;
                            }
                            await tick();
                            scrollToSample(samples[0].id);
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
                            // ENTERPRISE FIX: Auto-Play Prüfung auch beim Seitenwechsel
                            if (appState.autoPlayEnabled !== false) {
                                await handlePlayRequest(samples[samples.length - 1]);
                            } else {
                                selectedId = samples[samples.length - 1].id;
                            }
                            await tick();
                            scrollToSample(samples[samples.length - 1].id);
                        }
                    }
                    return;
                } else currentIndex--;
            }

            const nextSample = samples[currentIndex];

            // ENTERPRISE FIX: Respektiert die Auto-Play Einstellung für das normale Scrollen
            if (appState.autoPlayEnabled !== false) {
                handlePlayRequest(nextSample);
            } else {
                // Wenn Auto-Play aus ist, wird nur die Zeile markiert, aber nicht abgespielt
                selectedId = nextSample.id;
            }

            await tick();
            scrollToSample(nextSample.id);

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

    // Leitet den Mode ('min' | 'maj') aus einem key_signature String ab.
    // Unterstützt: "C min", "D# minor", "F maj", "G major", Camelot "8A"(min)/"8B"(maj).
    function parseKeyMode(keyStr: string): 'min' | 'maj' {
        const lower = keyStr.toLowerCase().trim();
        if (/\d+a$/.test(lower)) return 'min';    // Camelot: 8A → minor
        if (/\d+b$/.test(lower)) return 'maj';    // Camelot: 8B → major
        if (lower.includes('min')) return 'min';
        return 'maj'; // Default: Major (auch für reine Noten wie "C" ohne Mode-Angabe)
    }

    // Extrahiert nur die Grund-Note aus einem key_signature String.
    // "D# min" → "D#" | "Fmaj" → "F" | "8A" → chromatisch gelöst via Camelot-Map
    function parseKeyNote(keyStr: string): string {
        // Camelot-Format: Zahl + A/B → Note über Camelot-Wheel
        const camelotMatch = keyStr.match(/^(1[0-2]|[1-9])([AB])$/i);
        if (camelotMatch) {
            const CAMELOT_TO_NOTE: Record<string, string> = {
                '1A':'Ab','1B':'B','2A':'Eb','2B':'F#','3A':'Bb','3B':'Db',
                '4A':'F','4B':'Ab','5A':'C','5B':'Eb','6A':'G','6B':'Bb',
                '7A':'D','7B':'F','8A':'A','8B':'C','9A':'E','9B':'G',
                '10A':'B','10B':'D','11A':'F#','11B':'A','12A':'Db','12B':'E'
            };
            const key = camelotMatch[1].toUpperCase() + camelotMatch[2].toUpperCase();
            return CAMELOT_TO_NOTE[key] ?? '';
        }
        // Normales Format: erste Token ist die Note ("D#", "C", "F")
        return keyStr.trim().split(/\s+/)[0].toUpperCase();
    }

    /**
     * Berechnet den optimalen Pitch-Shift in Halbtönen zwischen einem Sample-Key
     * und einem Ziel-Key unter Berücksichtigung der Relative-Key-Logik:
     *
     *   Ziel = D min, Sample = F maj  →  0 Halbtöne (selbe Skala, Parallele)
     *   Ziel = D min, Sample = C maj  →  +5 Halbtöne (C→F, da Fmaj = Relativ-Major von Dmin)
     *   Ziel = F maj, Sample = D min  →  0 Halbtöne (selbe Skala)
     *
     * Relative-Major ist immer 3 Halbtöne ÜBER dem Moll-Grundton.
     * Relative-Minor ist immer 3 Halbtöne UNTER dem Dur-Grundton.
     */
    function getSemitoneShift(
        sampleKey: string | null,
        targetNote: string | null,
        targetMode: 'min' | 'maj'
    ): number {
        if (!sampleKey || !targetNote) return 0;

        const RELATIVE_SEMITONES = 3; // Durton liegt immer 3 HT über dem Mollton

        const sNote = parseKeyNote(sampleKey);
        const sMode = parseKeyMode(sampleKey);

        const sIdx = CHROMATIC_SCALE.indexOf(sNote);
        let tIdx  = CHROMATIC_SCALE.indexOf(targetNote.toUpperCase());

        if (sIdx === -1 || tIdx === -1) return 0;

        // Relative-Key-Anpassung: Wenn Sample und Ziel unterschiedliche Modi haben,
        // pitchen wir das Sample auf den Relativ-Key des Ziels (gleiche Skala, anderer Startpunkt).
        if (targetMode === 'min' && sMode === 'maj') {
            // Ziel: Moll → Relativ-Dur des Ziels = tNote + 3 HT
            // Beispiel: Ziel D min → Relativ-Dur = F maj → Major-Sample auf F pitchen
            tIdx = (tIdx + RELATIVE_SEMITONES) % 12;
        } else if (targetMode === 'maj' && sMode === 'min') {
            // Ziel: Dur → Relativ-Moll des Ziels = tNote − 3 HT
            // Beispiel: Ziel F maj → Relativ-Moll = D min → Minor-Sample auf D pitchen
            tIdx = (tIdx - RELATIVE_SEMITONES + 12) % 12;
        }
        // Gleicher Modus: keine Anpassung nötig

        let diff = tIdx - sIdx;

        // Shortest Path: Niemals mehr als 6 Halbtöne pitchen (natürlichster Klang)
        if (diff > 6)  diff -= 12;
        if (diff < -6) diff += 12;

        return diff;
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
            // Relative-Key-Logik: globalKeyMode bestimmt ob Dur- oder Moll-Samples
            // auf den Relativ-Key gepitched werden (gleiche Skala, anderer Startpunkt).
            semitones = getSemitoneShift(sample.key_signature, appState.globalKey, appState.globalKeyMode);
        }

        // BPM Stretcher: time-stretch sample to globalBpm without changing pitch.
        // Ratio 1.0 = identity (ssstretch fast-path, no processing overhead).
        const stretchRatio = getStretchRatio(sample);

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

    function goToPage(p: number) { if (p !== currentPage) { currentPage = p; loadSamples(); } }
    function nextPage() { if (currentPage < totalPages) { currentPage++; loadSamples(); } }
    function prevPage() { if (currentPage > 1) { currentPage--; loadSamples(); } }
    function toggleFilter(type: string) { activeTypeFilter = activeTypeFilter === type ? null : type; currentPage = 1; loadSamples(); }
    // ENTERPRISE FIX: Intelligentes Waveform-Downsampling
    function parseWaveform(data: number[] | null, targetLength: number = 40): number[] {
        if (!data || data.length === 0) return Array(targetLength).fill(10);
        if (data.length === targetLength) return data;

        const result = [];
        const step = data.length / targetLength;
        for (let i = 0; i < targetLength; i++) {
            const idx = Math.floor(i * step);
            result.push(data[Math.min(idx, data.length - 1)] || 10);
        }
        return result;
    }

    // --- BATCH SCANNING QUEUE ---
    let scanQueue: string[] = $state([]);
    let isQueueDropdownOpen = $state(false);

    let isSyncing: boolean = $state(false);
    let isScanningNew: boolean = $state(false);
    let isClearing: boolean = $state(false);
    let isScanning = $derived(isSyncing || isScanningNew || isClearing);
    let scanMessage: string = $state('');

    async function handleSelectFolder() {
        try {
            // Enterprise: Wir erlauben direkt die Mehrfachauswahl im OS-Dialog!
            const result = await open({ directory: true, multiple: true });
            if (result) {
                let added = 0;
                if (Array.isArray(result)) {
                    for (const path of result) {
                        if (!scanQueue.includes(path)) {
                            scanQueue = [...scanQueue, path];
                            added++;
                        }
                    }
                } else {
                    if (!scanQueue.includes(result as string)) {
                        scanQueue = [...scanQueue, result as string];
                        added++;
                    }
                }
                if (added > 0) scanMessage = `${added} folder(s) added to queue.`;
            }
        } catch (error) { console.error(error); }
    }

    async function handleScan() {
        if (scanQueue.length === 0) return;
        isScanningNew = true;
        let totalAdded = 0;

        // Kopiere die Queue, um sie abzuarbeiten
        const queueToProcess = [...scanQueue];

        for (const path of queueToProcess) {
            scanMessage = `Indexing ${path.split(/[/\\]/).pop()}...`;
            try {
                const count = await invoke<number>('scan_library', { path });
                totalAdded += count;
                // Ordner erfolgreich gescannt -> aus der Liste entfernen
                scanQueue = scanQueue.filter(p => p !== path);
            } catch (error) {
                console.error(`Error scanning ${path}:`, error);
            }
        }

        scanMessage = `Batch complete. Added ${totalAdded} files in total.`;
        currentPage = 1;
        await loadSamples();
        isScanningNew = false;

        setTimeout(() => { if (!isScanningNew) scanMessage = ''; }, 4000);
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
    // 'globalkey' ist das Piano-Dropdown für den globalen Pitch-Key im Player-Header
    let openDropdown: 'instrument' | 'genre' | 'key' | 'bpm' | 'format' | 'globalkey' | 'globalbpm' | null = $state(null);

    // Absoluter Pfad zum appDataDir — wird in onMount aufgelöst.
    // Drag-Icons werden dort als 64×64 PNGs gecacht.
    let _appDataPath = '';
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

    // --- GLOBAL KEY PIANO (Pitch-Shifter im Header) ---
    // Setzt den globalen Pitch-Key. Schließt das Dropdown nicht automatisch —
    // der User sieht sofort die Piano-Selektion und kann den Mode wechseln.
    function setGlobalPianoKey(note: string) {
        // Nochmals dieselbe Note → Toggle off
        if (appState.globalKey === note) {
            appState.globalKey = null;
        } else {
            appState.globalKey = note;
        }
    }

    function isGlobalKeyActive(note: string) { return appState.globalKey === note; }

    // Lesbarer Label für den Trigger-Button: "D min", "F# maj", "Off"
    let globalKeyLabel = $derived(
        appState.globalKey
            ? `${appState.globalKey} ${appState.globalKeyMode}`
            : 'Off'
    );
    // ─── BPM STRETCHER ───────────────────────────────────────────────────────────
    // Returns the playback speed ratio needed to hit the global BPM target.
    // Returns 1.0 when the sample has no BPM data or globalBpm is off.
    function getStretchRatio(sample: SampleRecord): number {
        if (!appState.globalBpm || !sample.bpm || sample.bpm <= 0) return 1.0;
        return appState.globalBpm / sample.bpm;
    }

    // Lesbarer Label für den BPM Trigger-Button: "120 BPM" oder "BPM"
    let globalBpmLabel = $derived(
        appState.globalBpm ? `${appState.globalBpm} BPM` : 'BPM'
    );

    function toggleTagMatchMode() { appState.filters.tagMatchMode = appState.filters.tagMatchMode === 'AND' ? 'OR' : 'AND'; currentPage = 1; loadSamples(); }

    let isTagsExpanded: boolean = $state(false);
    let sortedAvailableTags = $derived.by(() => {
        // HIER ÄNDERN:
        return [...activeDropdownTags].sort((a, b) => {
            const aActive = isTagActive(a.category, a.value) ? 1 : 0;
            const bActive = isTagActive(b.category, b.value) ? 1 : 0;
            if (aActive !== bActive) return bActive - aActive;
            return 0;
        });
    });

    function toggleDropdown(dropdown: 'instrument' | 'genre' | 'key' | 'bpm' | 'format' | 'globalkey' | 'globalbpm', event: Event) {
        event.stopPropagation();
        openDropdown = openDropdown === dropdown ? null : dropdown;
    }

    function toggleFilterTag(category: string, value: string) {
        if (category === 'Format') {
            // ENTERPRISE FIX: Radio-Button Logik für Formate (Exklusive Auswahl)
            // Wenn der Tag bereits aktiv ist, wird er deaktiviert.
            // Ansonsten wird er als einziger Tag in das Array gelegt (überschreibt vorherige).
            if (appState.filters.formats.includes(value)) {
                appState.filters.formats = [];
            } else {
                appState.filters.formats = [value];
            }
        } else {
            // Standard Checkbox-Logik für alle anderen Kategorien (Mehrfachauswahl)
            let targetArray: string[];
            if (category === 'Genre') targetArray = appState.filters.genres;
            else if (category === 'Key') targetArray = appState.filters.keys;
            else targetArray = appState.filters.instruments;

            const idx = targetArray.indexOf(value);
            if (idx > -1) targetArray.splice(idx, 1);
            else targetArray.push(value);

            if (category === 'Genre') appState.filters.genres = [...targetArray];
            else if (category === 'Key') appState.filters.keys = [...targetArray];
            else appState.filters.instruments = [...targetArray];
        }

        currentPage = 1;
        loadSamples();
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
        // ENTERPRISE FIX: Nativer, asynchroner OS-Dialog blockiert die UI nicht fehlerhaft!
        const confirmed = await ask(`Un-link this folder?\n\n${folderPath}\n\nThis will remove all its samples from your library.`, {
            title: 'SampleVault',
            kind: 'warning'
        });
        if (!confirmed) return;

        isSettingsLoading = true;
        try {
            await invoke('remove_folder', { path: folderPath });
            await loadConnectedFolders();
            currentPage = 1; await loadSamples();
        } catch (error) { console.error(error); }
        finally { isSettingsLoading = false; }
    }

    async function handleClearDatabase() {
        const confirmed = await ask("Clear the entire library? This action cannot be undone.", {
            title: 'SampleVault',
            kind: 'warning'
        });

        if (confirmed) {
            isClearing = true;
            try {
                // 1. Audio strikt stoppen
                if (appState.isPlaying) {
                    invoke('stop_audio').catch(console.error);
                    appState.isPlaying = false;
                    cancelAnimationFrame(animationFrameId);
                }

                // 2. Globalen Player-State restlos nullen (behebt den Ghost-Sound im Footer)
                appState.currentSample = null;
                selectedId = null;
                playingId = null;

                // 3. Datenbank leeren
                await invoke('clear_database');

                // 4. UI-Listen nullen (behebt das Stale-UI im Settings-Menü)
                activeTypeFilter = null;
                currentPage = 1;
                samples = [];
                totalItems = 0;
                appState.collections = [];
                appState.filters.collectionId = null;
                connectedFolders = []; // <--- WICHTIG FÜR SETTINGS TAB!

                scanMessage = 'Library cleared.';
            } catch (error) { scanMessage = `Error: ${error}`; }
            finally { isClearing = false; }
        }
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

    // Cache: sample.id → absoluter Pfad zum 64×64 Drag-Icon PNG
    const _dragIconCache = new Map<string, string>();

    /**
     * Erzeugt ein 64×64 PNG für den OS-Drag und cached es im appDataDir.
     * - Cover vorhanden → Cover auf 64×64 verkleinert (kein riesiges Bild mehr am Cursor)
     * - Kein Cover → generiertes Music-Note-Icon (dunkel + Emerald)
     *
     * Das Icon wird beim mousedown PARALLEL zur Drag-Geste erzeugt — by the time
     * der User 5px bewegt hat, ist es fertig. Kein spürbarer Overhead.
     */
    async function prepareDragIcon(sample: SampleRecord): Promise<string> {
        if (_dragIconCache.has(sample.id)) return _dragIconCache.get(sample.id)!;
        if (!_appDataPath) return '';

        const SIZE = 64;
        const canvas = document.createElement('canvas');
        canvas.width = SIZE; canvas.height = SIZE;
        const ctx = canvas.getContext('2d')!;

        let drewCover = false;
        if (sample.cover_path) {
            try {
                // WICHTIG: img.src = convertFileSrc(...) → asset://localhost/ Origin →
                // Canvas wird "tainted" → toBlob() wirft SecurityError.
                // Fix: Bild per fetch() als Blob laden → createObjectURL() erzeugt eine
                // blob:-URL (same-origin) → kein Taint → toBlob() funktioniert.
                const response = await fetch(convertFileSrc(sample.cover_path!));
                if (response.ok) {
                    const imgBlob   = await response.blob();
                    const objectUrl = URL.createObjectURL(imgBlob);
                    const img       = new Image();
                    await new Promise<void>((resolve) => {
                        img.onload  = () => resolve();
                        img.onerror = () => resolve();
                        img.src     = objectUrl;
                    });
                    URL.revokeObjectURL(objectUrl); // Sofort freigeben
                    if (img.naturalWidth > 0) {
                        ctx.save();
                        ctx.beginPath();
                        if (ctx.roundRect) ctx.roundRect(0, 0, SIZE, SIZE, 10);
                        else ctx.rect(0, 0, SIZE, SIZE); // Fallback für ältere WebViews
                        ctx.clip();
                        ctx.drawImage(img, 0, 0, SIZE, SIZE);
                        ctx.restore();
                        drewCover = true;
                    }
                }
            } catch { /* Fehler → Fallback-Icon */ }
        }

        if (!drewCover) {
            // Generiertes Icon: dunkler Hintergrund + Emerald Music-Note
            ctx.fillStyle = '#27272a';
            ctx.beginPath();
            if (ctx.roundRect) ctx.roundRect(0, 0, SIZE, SIZE, 10);
            else ctx.rect(0, 0, SIZE, SIZE);
            ctx.fill();
            ctx.fillStyle = '#22c55e';
            ctx.font = 'bold 38px serif';
            ctx.textAlign    = 'center';
            ctx.textBaseline = 'middle';
            ctx.fillText('♪', SIZE / 2, SIZE / 2 + 2);
        }

        // Canvas → PNG-Bytes → Datei
        const blob  = await new Promise<Blob>((resolve) => canvas.toBlob((b) => resolve(b!), 'image/png'));
        const bytes = new Uint8Array(await blob.arrayBuffer());
        const iconPath = _appDataPath + 'drag-icons/' + sample.id + '.png';

        await writeFile(iconPath, bytes);
        _dragIconCache.set(sample.id, iconPath);
        return iconPath;
    }

    // --- ENTERPRISE OS-DRAG & DROP ---
    // Kommuniziert direkt mit dem OS via @crabnebula/tauri-plugin-drag.
    // Umgeht den Browser-Drag komplett — der User kann das Sample in seine DAW ziehen.
    //
    // Icon-Strategie:
    //   prepareDragIcon() erzeugt beim mousedown PARALLEL ein 64×64 PNG (Cover oder
    //   generiertes Music-Note-Icon). By the time der User 5px bewegt hat, ist das
    //   Icon bereits fertig im Cache. Kein Overhead, kein riesiges Bild mehr am Cursor.
    function nativeDrag(node: HTMLElement, sampleArg: SampleRecord) {
        let sample     = sampleArg;
        let startX     = 0;
        let startY     = 0;
        let isDragging = false;                      // Guard gegen parallele startDrag()-Aufrufe
        let didDrag    = false;                      // Swallowed den nächsten click nach Drag
        let iconPromise: Promise<string> | null = null; // Startet beim mousedown, await im move

        const handleMouseMove = async (e: MouseEvent) => {
            if (isDragging) return;

            const dx = Math.abs(e.clientX - startX);
            const dy = Math.abs(e.clientY - startY);

            if (dx > 5 || dy > 5) {
                isDragging = true;
                cleanupWindowListeners();

                try {
                    // Icon wurde bereits beim mousedown vorbereitet — meistens schon fertig.
                    const icon = await (iconPromise ?? prepareDragIcon(sample));
                    await startDrag({ item: [sample.original_path], icon });
                    didDrag = true;
                } catch (err) {
                    console.error('[SampleVault] OS-Drag fehlgeschlagen:', err);
                } finally {
                    isDragging = false;
                }
            }
        };

        const handleMouseUp = () => {
            cleanupWindowListeners();
        };

        // Unterdrückt den click-Event der nach mousedown+mouseup noch gefeuert wird —
        // verhindert versehentliche Sample-Wiedergabe oder Selektion nach einem Drag.
        const handleClick = (e: MouseEvent) => {
            if (didDrag) {
                e.stopPropagation();
                e.preventDefault();
                didDrag = false;
            }
        };

        const cleanupWindowListeners = () => {
            window.removeEventListener('mousemove', handleMouseMove);
            window.removeEventListener('mouseup', handleMouseUp);
        };

        const handleMouseDown = (e: MouseEvent) => {
            if (e.button !== 0) return; // Nur Linksklick

            // preventDefault verhindert Browser-Standard-Drag (Text-Selektion, Image-Ghost).
            e.preventDefault();

            startX      = e.clientX;
            startY      = e.clientY;
            isDragging  = false;
            didDrag     = false;

            // Icon parallel starten — während der User 5px bewegt, wird das PNG generiert.
            // .catch(() => '') damit ein Fehler den Drag nicht blockiert.
            iconPromise = prepareDragIcon(sample).catch(() => '');

            window.addEventListener('mousemove', handleMouseMove);
            window.addEventListener('mouseup',   handleMouseUp);
        };

        node.addEventListener('mousedown', handleMouseDown);
        node.addEventListener('click',     handleClick);

        return {
            update(newSample: SampleRecord) {
                sample      = newSample;
                iconPromise = null; // Cache-Referenz zurücksetzen — neues Sample, neues Icon
            },
            destroy() {
                node.removeEventListener('mousedown', handleMouseDown);
                node.removeEventListener('click',     handleClick);
                cleanupWindowListeners();
            }
        };
    }

    // --- ENTERPRISE WAVEFORM ZOOM & CLICK ---
    let editorZoomLevel = $state(1.0);
    let editorScrollContainer: HTMLDivElement;

    function handleZoomIn() {
        editorZoomLevel = Math.min(10.0, editorZoomLevel * 1.5);
    }

    function handleZoomOut() {
        editorZoomLevel = Math.max(1.0, editorZoomLevel / 1.5);
    }

    // Die neue Funktion für das Click-to-Snap Feature
    function handleWaveformClick(e: MouseEvent) {
        if (isSliceReady || !editorWaveformContainer) return;

        const rect = editorWaveformContainer.getBoundingClientRect();
        let pct = (e.clientX - rect.left) / rect.width;
        pct = Math.max(0, Math.min(1, pct));

        // Spaltet den Editor logisch in zwei Hälften (0.5).
        // Klick links = Start-Locator springt. Klick rechts = End-Locator springt.
        if (pct < 0.5) {
            trimStartPct = Math.min(pct, trimEndPct - 0.02);
            currentPreviewPath = null;
            if (appState.isPlaying) playSlicePreview(true);
        } else {
            trimEndPct = Math.max(pct, trimStartPct + 0.02);
        }
    }

    function editorWheelZoom(node: HTMLElement) {
        const handleWheel = (e: WheelEvent) => {
            if (e.ctrlKey || e.metaKey) {
                e.preventDefault();

                if (!editorScrollContainer || !editorWaveformContainer) return;

                const zoomFactor = 1.1;
                const isZoomingIn = e.deltaY < 0;
                const oldZoom = editorZoomLevel;

                let newZoom = isZoomingIn ? oldZoom * zoomFactor : oldZoom / zoomFactor;
                // ENTERPRISE FIX: Max Zoom auf 10x limitiert (verhindert den "Grünen Block"-Fehler)
                newZoom = Math.max(1.0, Math.min(10.0, newZoom));

                if (newZoom === oldZoom) return;

                const rect = editorWaveformContainer.getBoundingClientRect();
                const mouseXRel = e.clientX - rect.left;
                const mousePct = mouseXRel / rect.width;

                editorZoomLevel = newZoom;

                tick().then(() => {
                    const newRect = editorWaveformContainer.getBoundingClientRect();
                    const newMouseXRel = mousePct * newRect.width;
                    const scrollOffset = e.clientX - editorScrollContainer.getBoundingClientRect().left;
                    editorScrollContainer.scrollLeft = newMouseXRel - scrollOffset;
                });
            }
        };

        node.addEventListener('wheel', handleWheel, { passive: false });
        return { destroy() { node.removeEventListener('wheel', handleWheel); } };
    }

    // --- SAMPLER & TRIMMER STATE ---
    let isSamplerOpen = $state(false);
    let samplerSample: SampleRecord | null = $state(null);

    // Wir trennen strikt zwischen Preview (immer bis 100% Länge) und Export (genau geschnitten)
    let currentPreviewPath: string | null = $state(null);
    let exportSlicedPath: string | null = $state(null);
    let previewSliceStartPct = $state(0);

    let isSlicing = $state(false);
    let isSliceReady = $state(false);
    let isLooping = $state(false);

    let trimStartPct = $state(0);
    let trimEndPct = $state(1);
    let isDraggingHandle: 'start' | 'end' | null = $state(null);
    let editorWaveformContainer: HTMLDivElement;

    function openSampler(sample: SampleRecord) {

        editorZoomLevel = 1.0;

        if (appState.isPlaying) {
            invoke('stop_audio').catch(console.error);
            appState.isPlaying = false;
            playingId = null;
            cancelAnimationFrame(animationFrameId);
        }

        samplerSample = sample;
        isSamplerOpen = true;
        isSliceReady = false;
        trimStartPct = 0;
        trimEndPct = 1;
        currentPreviewPath = null;
        exportSlicedPath = null;
    }

    function closeSampler() {
        if (appState.isPlaying) {
            invoke('stop_audio').catch(console.error);
            appState.isPlaying = false;
            playingId = null;
            cancelAnimationFrame(animationFrameId);
        }

        isSamplerOpen = false;
        samplerSample = null;
        currentPreviewPath = null;
        exportSlicedPath = null;
        isSliceReady = false;
    }

    async function confirmSlice() {
        if (!samplerSample) return;
        isSlicing = true;
        try {
            // ENTERPRISE FIX: Für den Export (Drag & Drop) schneiden wir exakt!
            const startMs = trimStartPct * samplerSample.duration_ms;
            const endMs = trimEndPct * samplerSample.duration_ms;
            exportSlicedPath = await invoke<string>('slice_audio', {
                path: samplerSample.original_path,
                startMs: startMs,
                endMs: endMs
            });
            isSliceReady = true;
        } catch (e) {
            console.error("Failed to slice audio:", e);
        } finally {
            isSlicing = false;
        }
    }

    function editSlice() {
        isSliceReady = false;
        exportSlicedPath = null;
    }

    // ENTERPRISE PREVIEW: Generiert immer bis 100% Datei-Ende, um Stille beim Ziehen zu verhindern!
    async function playSlicePreview(forceRestart = false) {
        if (!samplerSample) return;

        if (appState.isPlaying && !forceRestart) {
            invoke('stop_audio').catch(console.error);
            appState.isPlaying = false;
            cancelAnimationFrame(animationFrameId);
            return;
        }

        if (isSlicing && !currentPreviewPath) return;

        // Neu berechnen, wenn noch kein File da ist, ODER der Startpunkt verschoben wurde
        if (!isSliceReady && (!currentPreviewPath || previewSliceStartPct !== trimStartPct)) {
            isSlicing = true;
            try {
                const startMs = trimStartPct * samplerSample.duration_ms;
                const endMs = samplerSample.duration_ms; // IMMER BIS ZUM ABSOLUTEN ENDE!
                currentPreviewPath = await invoke<string>('slice_audio', {
                    path: samplerSample.original_path,
                    startMs: startMs,
                    endMs: endMs
                });
                previewSliceStartPct = trimStartPct;
            } catch (e) {
                console.error("Preview error:", e);
                return;
            } finally {
                isSlicing = false;
            }
        }

        if (currentPreviewPath) {
            let semitones = 0;
            if (appState.globalKey && samplerSample.key_signature) {
                semitones = getSemitoneShift(samplerSample.key_signature, appState.globalKey, appState.globalKeyMode);
            }

            try {
                if (appState.isPlaying) await invoke('stop_audio');
                await invoke('play_audio', {
                    filePath: currentPreviewPath,
                    semitones: semitones,
                    stretchRatio: getStretchRatio(samplerSample),
                    volume: appState.globalVolume,
                    speed: appState.vinylSpeedMode // <--- ÜBERGABE AN RUST
                });
                appState.isPlaying = true;
                playbackStartTime = performance.now();
                animationFrameId = requestAnimationFrame(updateProgress);
            } catch (error) {
                console.error(error);
            }
        }
    }

    function handleEditorMouseMove(e: MouseEvent) {
        if (!isDraggingHandle || !editorWaveformContainer || isSliceReady) return;
        const rect = editorWaveformContainer.getBoundingClientRect();
        let pct = (e.clientX - rect.left) / rect.width;
        pct = Math.max(0, Math.min(1, pct));

        if (isDraggingHandle === 'start') {
            trimStartPct = Math.min(pct, trimEndPct - 0.02);
            currentPreviewPath = null; // Zerstört den Cache nur, wenn der Start verschoben wird
        } else {
            trimEndPct = Math.max(pct, trimStartPct + 0.02);
            // End-Änderungen lassen das laufende Audio völlig unangetastet!
        }
    }

    function handleEditorMouseUp() {
        if (isDraggingHandle) {
            const wasStart = isDraggingHandle === 'start';
            isDraggingHandle = null;
            // Triggert einen neuen Schnitt/Neustart NUR, wenn der linke Start-Regler angefasst wurde.
            // Der rechte Regler ist ab sofort zu 100% dynamisch über die Engine gesteuert!
            if (wasStart && appState.isPlaying && !isSliceReady) {
                playSlicePreview(true);
            }
        }
    }

    // --- ENTERPRISE INSTANT SLICE DRAG ---
    function nativeSliceDrag(node: HTMLElement) {
        let startX = 0;
        let startY = 0;
        let isDragging = false;
        let didDrag = false;

        const handleMouseMove = async (e: MouseEvent) => {
            // Drag ist NUR erlaubt, wenn der Nutzer auf "Confirm Selection" gedrückt hat!
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
                } catch (err) {
                    console.error("Slice & Drag Error:", err);
                } finally {
                    isDragging = false;
                }
            }
        };

        const handleMouseUp = () => cleanupWindowListeners();
        const handleClick = (e: MouseEvent) => { if (didDrag) { e.stopPropagation(); e.preventDefault(); didDrag = false; } };
        const cleanupWindowListeners = () => {
            window.removeEventListener('mousemove', handleMouseMove);
            window.removeEventListener('mouseup', handleMouseUp);
        };

        const handleMouseDown = (e: MouseEvent) => {
            if (e.button !== 0 || !isSliceReady) return;
            e.preventDefault();
            startX = e.clientX;
            startY = e.clientY;
            isDragging = false;
            didDrag = false;
            window.addEventListener('mousemove', handleMouseMove);
            window.addEventListener('mouseup', handleMouseUp);
        };

        node.addEventListener('mousedown', handleMouseDown);
        node.addEventListener('click', handleClick);

        return {
            destroy() {
                node.removeEventListener('mousedown', handleMouseDown);
                node.removeEventListener('click', handleClick);
                cleanupWindowListeners();
            }
        };
    }

    function handleForceRetrigger() {
        if (appState.currentSample && appState.isPlaying) {
            if (isSamplerOpen) playSlicePreview(true); // Neustart im Editor
            else handlePlayRequest(appState.currentSample, true); // Neustart in Liste
        }
    }

    // --- ENTERPRISE BULK TAGGING STATE ---
    let isBulkTagDropdownOpen = $state(false);
    let bulkTagSearchQuery = $state('');
    let isBulkTagging = $state(false);

    // 1. Mappt die ausgewählten IDs zu echten SampleRecords
    let selectedSampleRecords = $derived(samples.filter(s => appState.selectedSampleIds.includes(s.id)));

    // 2. Errechnet blitzschnell die Schnittmenge (Tags, die ALLE ausgewählten Samples gemeinsam haben)
    let commonTags = $derived.by(() => {
        if (selectedSampleRecords.length === 0) return [];

        let shared = parseTags(selectedSampleRecords[0].tags);
        for (let i = 1; i < selectedSampleRecords.length; i++) {
            const currentTags = parseTags(selectedSampleRecords[i].tags);
            shared = shared.filter(st => currentTags.some(ct => ct.category === st.category && ct.value === st.value));
        }
        return shared;
    });

    let filteredBulkTags = $derived(
        allAvailableTags.filter(t => t.value.toLowerCase().includes(bulkTagSearchQuery.toLowerCase()))
    );

    // --- ENTERPRISE BULK TAG PROCESSING ---
    async function handleBulkAddTag(category: string, value: string) {
        isBulkTagDropdownOpen = false;
        bulkTagSearchQuery = '';
        isBulkTagging = true;

        const tagToAdd = { category, value };
        const updates = [];

        // Optimistic UI Update: Wir updaten den lokalen State sofort
        for (let sample of selectedSampleRecords) {
            let currentTags = parseTags(sample.tags);

            if (!currentTags.some(t => t.category === category && t.value === value)) {
                currentTags.push(tagToAdd);
                sample.tags = JSON.stringify(currentTags);

                updates.push(invoke('update_sample_metadata', {
                    payload: {
                        id: sample.id,
                        filename: sample.filename,
                        bpm: sample.bpm,
                        keySignature: sample.key_signature,
                        tags: sample.tags
                    }
                }));
            }
        }

        samples = [...samples]; // Triggert das UI Rerendering sofort

        try {
            await Promise.all(updates); // Feuert alle Rust-Updates asynchron im Hintergrund ab
        } catch(e) { console.error("Bulk tag add failed:", e); }
        finally { isBulkTagging = false; }
    }

    async function handleBulkRemoveTag(category: string, value: string) {
        isBulkTagging = true;
        const updates = [];

        for (let sample of selectedSampleRecords) {
            let currentTags = parseTags(sample.tags);
            const originalLength = currentTags.length;

            currentTags = currentTags.filter(t => !(t.category === category && t.value === value));

            if (currentTags.length < originalLength) {
                sample.tags = JSON.stringify(currentTags);
                updates.push(invoke('update_sample_metadata', {
                    payload: {
                        id: sample.id,
                        filename: sample.filename,
                        bpm: sample.bpm,
                        keySignature: sample.key_signature,
                        tags: sample.tags
                    }
                }));
            }
        }

        samples = [...samples];

        try {
            await Promise.all(updates);
        } catch(e) { console.error("Bulk tag remove failed:", e); }
        finally { isBulkTagging = false; }
    }

    // --- ENTERPRISE BULK TAG CREATION ---
    async function createNewBulkTag() {
        const trimmed = bulkTagSearchQuery.trim();
        if (trimmed === '') return;

        try {
            // 1. Rust anweisen, den Tag global in der DB zu registrieren
            await invoke('create_user_tag', { category: 'User', value: trimmed });

            // 2. Das globale Frontend-Lexikon aktualisieren
            await loadAllTags();

            // 3. Den neuen Tag direkt auf alle ausgewählten Samples anwenden
            await handleBulkAddTag('User', trimmed);
        } catch (e) {
            console.error("Failed to create bulk tag:", e);
        }
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
                            <button onclick={handleRescanAll} disabled={isScanning} class="flex h-8 items-center gap-1.5 rounded-md border border-blue-700/50 bg-blue-50 px-3 text-xs font-medium text-blue-700 hover:bg-blue-100 dark:border-blue-500/30 dark:bg-blue-500/10 dark:text-blue-400 cursor-pointer disabled:opacity-50 transition-colors"><RefreshCw size={14} class={isSyncing ? "animate-spin" : ""} /> Sync</button>

                            <div class="flex items-center">
                                <button onclick={handleSelectFolder} disabled={isScanning} class="flex h-8 items-center gap-1.5 rounded-md border border-zinc-200 bg-white px-3 text-xs font-medium hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-800 dark:hover:bg-zinc-700 transition-colors disabled:opacity-50 cursor-pointer z-10 shadow-sm"><FolderPlus size={14} /> Add Folder</button>

                                {#if scanQueue.length > 0}
                                    <div class="relative ml-2 flex h-8 items-center shadow-sm rounded-md animate-in fade-in zoom-in-95 duration-200">

                                        <button onclick={handleScan} disabled={isScanning} class="flex h-full items-center gap-1.5 rounded-l-md border border-emerald-700/50 bg-emerald-50 px-3 text-xs font-bold text-emerald-700 hover:bg-emerald-100 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-400 cursor-pointer disabled:opacity-50 transition-colors">
                                            {#if isScanningNew}
                                                <RefreshCw size={14} class="animate-spin" /> Scanning...
                                            {:else}
                                                <Play size={14} /> Scan {scanQueue.length} {scanQueue.length === 1 ? 'Folder' : 'Folders'}
                                            {/if}
                                        </button>

                                        <button onclick={(e) => { e.stopPropagation(); isQueueDropdownOpen = !isQueueDropdownOpen; }} disabled={isScanning} class="flex h-full items-center justify-center border-y border-r border-emerald-700/50 bg-emerald-50 px-1.5 text-emerald-700 hover:bg-emerald-100 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-400 rounded-r-md cursor-pointer transition-colors disabled:opacity-50">
                                            <ChevronDown size={14} />
                                        </button>

                                        {#if isQueueDropdownOpen}
                                            <div onclick={(e) => e.stopPropagation()} class="absolute right-0 top-full mt-2 w-72 flex-col rounded-xl border border-zinc-200 bg-white p-1.5 shadow-2xl dark:border-zinc-700/60 dark:bg-[#18181b] z-50 flex">
                                                <div class="flex items-center justify-between px-2 pb-2 pt-1 border-b border-zinc-100 dark:border-zinc-800">
                                                    <span class="text-[10px] font-bold uppercase tracking-wider text-zinc-500">Scan Queue</span>
                                                    <button onclick={() => { scanQueue = []; isQueueDropdownOpen = false; }} class="text-[10px] text-red-500 hover:text-red-600 uppercase font-bold tracking-wider cursor-pointer">Clear All</button>
                                                </div>
                                                <div class="max-h-48 overflow-y-auto no-scrollbar flex flex-col gap-0.5 mt-1.5">
                                                    {#each scanQueue as path}
                                                        <div class="group flex items-center justify-between rounded-md px-2 py-1.5 text-xs hover:bg-zinc-50 dark:hover:bg-zinc-800 transition-colors">
                                                            <div class="flex items-center gap-2 overflow-hidden">
                                                                <Folder size={12} class="text-zinc-400 shrink-0" />
                                                                <span class="truncate font-medium text-zinc-700 dark:text-zinc-300" title={path}>{path.split(/[/\\]/).pop()}</span>
                                                            </div>
                                                            <button onclick={() => { scanQueue = scanQueue.filter(p => p !== path); if(scanQueue.length === 0) isQueueDropdownOpen = false; }} class="opacity-0 group-hover:opacity-100 text-zinc-400 hover:text-red-500 transition-all cursor-pointer shrink-0 ml-2">
                                                                <X size={12} />
                                                            </button>
                                                        </div>
                                                    {/each}
                                                </div>
                                            </div>
                                        {/if}
                                    </div>
                                {/if}
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            {#if appState.activeSoundsTab === 'collections' && appState.filters.collectionId === null && !appState.filters.onlyLiked}
                <div class="pl-8 pr-8 pb-8 pt-4 w-full">
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
                <div class="pb-8">
                    <div class="pl-8 pr-8 w-full">
                        <div class="mb-6 space-y-3">
                            <div class="flex flex-wrap items-center gap-2">
                                <div class="relative">
                                    <button onclick={(e) => toggleDropdown('instrument', e)} class="flex h-8 items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 text-xs font-semibold transition-colors cursor-pointer {appState.filters.instruments.length > 0 ? 'border-zinc-900 dark:border-zinc-100 text-zinc-900 dark:text-zinc-100' : 'text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800'}">
                                        Instrument {#if appState.filters.instruments.length > 0}<span class="flex h-4 w-4 items-center justify-center rounded-full bg-zinc-900 text-[9px] text-white dark:bg-zinc-100 dark:text-zinc-900">{appState.filters.instruments.length}</span>{/if}<ChevronDown size={14} class="opacity-50" />
                                    </button>
                                    <div onclick={(e) => e.stopPropagation()} class="absolute left-0 top-full mt-1 {openDropdown === 'instrument' ? 'flex' : 'hidden'} w-48 flex-col rounded-lg border border-zinc-200 bg-white p-1 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                                        <div class="max-h-60 overflow-y-auto no-scrollbar flex flex-col gap-0.5">
                                            {#each activeDropdownTags.filter(t => !['Genre', 'Format', 'Key', 'Character'].includes(t.category)) as tag}
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
                                            {#each activeDropdownTags.filter(t => t.category === 'Genre') as tag}
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
                                            {#each activeDropdownTags.filter(t => t.category === 'Format') as tag}
                                                <label class="flex items-center gap-2 rounded-md px-2 py-1.5 text-xs hover:bg-zinc-100 dark:hover:bg-zinc-800 cursor-pointer"><input type="checkbox" checked={isTagActive('Format', tag.value)} onchange={() => toggleFilterTag('Format', tag.value)} class="rounded border-zinc-300 dark:border-zinc-700 accent-zinc-900 dark:accent-zinc-100 cursor-pointer"> {tag.value}</label>
                                            {/each}
                                        </div>
                                    </div>
                                </div>

                                {#if appState.filters.instruments.length > 0 || appState.filters.genres.length > 0 || appState.filters.formats.length > 0 || appState.filters.keys.length > 0 || appState.filters.bpm.exact || appState.filters.bpm.min || appState.filters.bpm.max}
                                    <button onclick={clearAllFilters} class="ml-2 flex h-8 items-center text-xs font-semibold text-red-600 hover:text-red-700 dark:text-red-400 dark:hover:text-red-300 transition-colors cursor-pointer">Clear all</button>
                                {/if}

                                <div class="mx-1 h-5 w-px bg-zinc-300 dark:bg-zinc-700 shrink-0"></div>

                                <div class="relative shrink-0">
                                    <button
                                            onclick={(e) => { e.stopPropagation(); openDropdown = openDropdown === 'globalkey' ? null : 'globalkey'; }}
                                            class="flex h-8 items-center gap-1.5 rounded-full border px-3 text-xs font-bold transition-all cursor-pointer
                                               {appState.globalKey
                                                   ? 'border-emerald-500 bg-emerald-500 text-white shadow-sm shadow-emerald-500/30 dark:shadow-emerald-500/20'
                                                   : 'border-emerald-500/40 bg-emerald-50 text-emerald-700 hover:border-emerald-500 hover:bg-emerald-100 dark:border-emerald-500/20 dark:bg-emerald-500/5 dark:text-emerald-400 dark:hover:border-emerald-500/50 dark:hover:bg-emerald-500/10'}"
                                    >
                                        <Music2 size={13} class="shrink-0 {appState.globalKey ? 'opacity-100' : 'opacity-70'}" />
                                        <span>{globalKeyLabel}</span>
                                        {#if !appState.globalKey}
                                            <ChevronDown size={12} class="opacity-50" />
                                        {:else}
                                            <span class="relative flex h-1.5 w-1.5 shrink-0">
                                                <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-white opacity-75"></span>
                                                <span class="relative inline-flex rounded-full h-1.5 w-1.5 bg-white"></span>
                                            </span>
                                        {/if}
                                    </button>

                                    {#if openDropdown === 'globalkey'}
                                        <div
                                                onclick={(e) => e.stopPropagation()}
                                                class="absolute right-0 top-full mt-2 flex w-72 flex-col gap-3 rounded-xl border border-zinc-200 bg-white p-3 shadow-2xl dark:border-zinc-800 dark:bg-[#18181b] z-50"
                                        >
                                            <div class="flex items-center gap-2 pb-1 border-b border-zinc-100 dark:border-zinc-800">
                                                <Music2 size={13} class="text-emerald-500 shrink-0" />
                                                <span class="text-[11px] font-bold uppercase tracking-wider text-zinc-500 dark:text-zinc-400">Auto-Pitch Key</span>
                                            </div>

                                            <div class="flex rounded-lg bg-zinc-100 p-1 dark:bg-zinc-800/50 border border-zinc-200 dark:border-zinc-700/50">
                                                <button
                                                        onclick={() => { appState.globalKeyMode = 'min'; }}
                                                        class="flex-1 py-1.5 text-xs font-semibold rounded-md transition-all cursor-pointer
                                                           {appState.globalKeyMode === 'min'
                                                               ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white'
                                                               : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}"
                                                >Minor</button>
                                                <button
                                                        onclick={() => { appState.globalKeyMode = 'maj'; }}
                                                        class="flex-1 py-1.5 text-xs font-semibold rounded-md transition-all cursor-pointer
                                                           {appState.globalKeyMode === 'maj'
                                                               ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white'
                                                               : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}"
                                                >Major</button>
                                            </div>

                                            <div class="relative flex w-full h-24 rounded-lg border border-zinc-300 dark:border-zinc-700 overflow-hidden select-none">
                                                {#each whiteKeys as note}
                                                    <button
                                                            onclick={() => setGlobalPianoKey(note)}
                                                            class="flex-1 flex items-end justify-center pb-2 text-[10px] font-bold border-r border-zinc-200 dark:border-zinc-700 last:border-0 transition-colors cursor-pointer
                                                               {isGlobalKeyActive(note)
                                                                   ? 'bg-emerald-100 dark:bg-emerald-900/40 text-emerald-700 dark:text-emerald-300'
                                                                   : 'bg-white dark:bg-zinc-800 text-zinc-400 hover:bg-zinc-50 dark:hover:bg-zinc-700'}"
                                                    >{note}</button>
                                                {/each}
                                                {#each blackKeys as bk}
                                                    <button
                                                            onclick={() => setGlobalPianoKey(bk.note)}
                                                            style="left: {bk.left}; transform: translateX(-50%);"
                                                            class="absolute top-0 w-[9%] h-14 rounded-b flex items-end justify-center pb-1.5 text-[8px] font-bold transition-colors cursor-pointer z-10
                                                               {isGlobalKeyActive(bk.note)
                                                                   ? 'bg-emerald-600 text-white shadow-inner'
                                                                   : 'bg-zinc-900 text-zinc-300 hover:bg-zinc-700 dark:bg-black dark:hover:bg-zinc-900'}"
                                                    >{bk.note}</button>
                                                {/each}
                                            </div>

                                            {#if appState.globalKey}
                                                <p class="text-[10px] text-zinc-400 dark:text-zinc-500 leading-relaxed">
                                                    Samples in <span class="font-semibold text-emerald-600 dark:text-emerald-400">{globalKeyLabel}</span> werden automatisch gepitched. {appState.globalKeyMode === 'min' ? 'Major-Samples' : 'Minor-Samples'} landen auf dem Relativ-{appState.globalKeyMode === 'min' ? 'Dur' : 'Moll'}.
                                                </p>
                                            {/if}

                                            <button
                                                    onclick={() => { appState.globalKey = null; openDropdown = null; }}
                                                    class="w-full rounded-lg py-1.5 text-xs font-semibold text-zinc-400 hover:bg-zinc-100 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-300 transition-colors cursor-pointer border border-transparent hover:border-zinc-200 dark:hover:border-zinc-700"
                                            >Turn Off</button>
                                        </div>
                                    {/if}
                                </div>

                                <!-- BPM Pill Divider -->
                                <div class="mx-1 h-5 w-px bg-zinc-300 dark:bg-zinc-700 shrink-0"></div>

                                <!-- ── BPM Stretcher Pill ── -->
                                <div class="relative shrink-0">
                                    <button
                                            onclick={(e) => { e.stopPropagation(); openDropdown = openDropdown === 'globalbpm' ? null : 'globalbpm'; }}
                                            class="flex h-8 items-center gap-1.5 rounded-full border px-3 text-xs font-bold transition-all cursor-pointer
                                               {appState.globalBpm
                                                   ? 'border-violet-500 bg-violet-500 text-white shadow-sm shadow-violet-500/30 dark:shadow-violet-500/20'
                                                   : 'border-violet-500/40 bg-violet-50 text-violet-700 hover:border-violet-500 hover:bg-violet-100 dark:border-violet-500/20 dark:bg-violet-500/5 dark:text-violet-400 dark:hover:border-violet-500/50 dark:hover:bg-violet-500/10'}"
                                    >
                                        <Gauge size={13} class="shrink-0 {appState.globalBpm ? 'opacity-100' : 'opacity-70'}" />
                                        <span>{globalBpmLabel}</span>
                                        {#if !appState.globalBpm}
                                            <ChevronDown size={12} class="opacity-50" />
                                        {:else}
                                            <span class="relative flex h-1.5 w-1.5 shrink-0">
                                                <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-white opacity-75"></span>
                                                <span class="relative inline-flex rounded-full h-1.5 w-1.5 bg-white"></span>
                                            </span>
                                        {/if}
                                    </button>

                                    {#if openDropdown === 'globalbpm'}
                                        <div
                                                onclick={(e) => e.stopPropagation()}
                                                class="absolute left-0 top-full mt-2 w-64 flex-col gap-3 rounded-xl border border-zinc-200 bg-white p-3 shadow-2xl dark:border-zinc-700/60 dark:bg-[#18181b] z-50 flex"
                                        >
                                            <!-- Header -->
                                            <div class="flex items-center gap-2 border-b border-zinc-100 pb-2.5 dark:border-zinc-800">
                                                <Gauge size={13} class="text-violet-500 shrink-0" />
                                                <span class="text-xs font-bold text-zinc-700 dark:text-zinc-200">BPM Stretcher</span>
                                            </div>

                                            <!-- BPM Input with +/- controls -->
                                            <div class="flex items-center gap-2">
                                                <button
                                                        onclick={() => { if (appState.globalBpm && appState.globalBpm > 1) appState.globalBpm = Math.max(1, appState.globalBpm - 1); }}
                                                        class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-zinc-200 bg-zinc-50 text-sm font-bold text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700 transition-colors cursor-pointer select-none"
                                                >−</button>
                                                <input
                                                        type="number"
                                                        min="1"
                                                        max="300"
                                                        placeholder="e.g. 120"
                                                        value={appState.globalBpm ?? ''}
                                                        oninput={(e) => {
                                                            const v = parseInt((e.target as HTMLInputElement).value);
                                                            appState.globalBpm = (!isNaN(v) && v > 0) ? v : null;
                                                        }}
                                                        class="h-8 flex-1 rounded-lg border border-zinc-200 bg-zinc-50 px-3 text-center text-sm font-bold text-zinc-900 focus:border-violet-500 focus:outline-none focus:ring-1 focus:ring-violet-500/30 dark:border-zinc-700 dark:bg-zinc-800 dark:text-white dark:focus:border-violet-500 transition-colors"
                                                />
                                                <button
                                                        onclick={() => { appState.globalBpm = (appState.globalBpm ?? 119) + 1; }}
                                                        class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-zinc-200 bg-zinc-50 text-sm font-bold text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700 transition-colors cursor-pointer select-none"
                                                >+</button>
                                            </div>

                                            <!-- Common BPM presets -->
                                            <div>
                                                <p class="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">Presets</p>
                                                <div class="grid grid-cols-5 gap-1">
                                                    {#each [80, 90, 100, 110, 120, 128, 140, 150, 160, 174] as preset}
                                                        <button
                                                                onclick={() => { appState.globalBpm = preset; }}
                                                                class="rounded-md py-1.5 text-xs font-semibold transition-colors cursor-pointer
                                                                   {appState.globalBpm === preset
                                                                       ? 'bg-violet-500 text-white shadow-sm'
                                                                       : 'bg-zinc-100 text-zinc-600 hover:bg-violet-50 hover:text-violet-700 dark:bg-zinc-800 dark:text-zinc-400 dark:hover:bg-violet-500/10 dark:hover:text-violet-300'}"
                                                        >{preset}</button>
                                                    {/each}
                                                </div>
                                            </div>

                                            <!-- Hint -->
                                            {#if appState.globalBpm}
                                                <p class="text-[10px] text-zinc-400 dark:text-zinc-500 leading-relaxed">
                                                    Samples mit bekanntem BPM werden auf <span class="font-semibold text-violet-600 dark:text-violet-400">{appState.globalBpm} BPM</span> gestreckt — ohne Pitch-Änderung.
                                                </p>
                                            {:else}
                                                <p class="text-[10px] text-zinc-400 dark:text-zinc-500 leading-relaxed">
                                                    Wähle ein Ziel-BPM. Nur Samples mit bekanntem BPM werden beeinflusst.
                                                </p>
                                            {/if}

                                            <!-- Turn Off -->
                                            <button
                                                    onclick={() => { appState.globalBpm = null; openDropdown = null; }}
                                                    class="w-full rounded-lg py-1.5 text-xs font-semibold text-zinc-400 hover:bg-zinc-100 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-300 transition-colors cursor-pointer border border-transparent hover:border-zinc-200 dark:hover:border-zinc-700"
                                            >Turn Off</button>
                                        </div>
                                    {/if}
                                </div>

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
                                    {#if hasActiveFilters}
                                        <div class="flex flex-col items-center justify-center py-32 text-zinc-500">
                                            <Search size={48} class="mb-4 opacity-20" />
                                            <span class="text-sm font-medium">No samples found matching these filters.</span>
                                            <button onclick={clearAllFilters} class="mt-4 text-xs font-semibold text-zinc-900 dark:text-zinc-100 underline cursor-pointer hover:opacity-70 transition-opacity">Clear all filters</button>
                                        </div>
                                    {:else}
                                        <div class="w-full px-0 pb-0 mt-8">
                                            <div class="flex w-full flex-col items-center justify-center py-24 text-zinc-500 border-2 border-dashed border-zinc-200 dark:border-zinc-800/60 rounded-2xl bg-zinc-50/50 dark:bg-[#121212]/50 transition-all">
                                                <div class="h-16 w-16 bg-zinc-100 dark:bg-zinc-800 rounded-2xl flex items-center justify-center mb-6 shadow-sm border border-zinc-200 dark:border-zinc-700">
                                                    <Library size={28} class="text-zinc-400" />
                                                </div>
                                                <h2 class="text-xl font-bold text-zinc-900 dark:text-white mb-2">Your library is empty</h2>
                                                <p class="text-sm text-zinc-500 mb-8 max-w-md text-center leading-relaxed">To view and manage your sounds, connect the folder where you store your music production files.</p>

                                                <div class="flex items-center gap-3">
                                                    <button onclick={handleSelectFolder} disabled={isScanning} class="flex items-center gap-2 rounded-lg bg-zinc-900 px-6 py-3 text-sm font-bold text-white shadow-sm hover:bg-zinc-800 dark:bg-white dark:text-zinc-900 dark:hover:bg-zinc-200 transition-colors disabled:opacity-50 cursor-pointer">
                                                        <FolderPlus size={18} /> Select Folders
                                                    </button>

                                                    {#if scanQueue.length > 0}
                                                        <button onclick={handleScan} disabled={isScanning} class="flex items-center gap-2 rounded-lg bg-emerald-500 px-6 py-3 text-sm font-bold text-white shadow-sm hover:bg-emerald-600 transition-colors disabled:opacity-50 cursor-pointer animate-in fade-in slide-in-from-left-4 duration-300">
                                                            {#if isScanningNew}
                                                                <RefreshCw size={18} class="animate-spin" /> Scanning...
                                                            {:else}
                                                                <Play size={18} /> Scan {scanQueue.length} {scanQueue.length === 1 ? 'Folder' : 'Folders'}
                                                            {/if}
                                                        </button>
                                                    {/if}
                                                </div>

                                                {#if scanMessage && isScanningNew}
                                                    <p class="mt-4 text-xs font-semibold text-emerald-600 dark:text-emerald-400 animate-pulse">{scanMessage}</p>
                                                {/if}

                                                {#if scanQueue.length > 0}
                                                    <div class="mt-10 w-full max-w-lg border-t border-zinc-200 dark:border-zinc-800/60 pt-6 animate-in fade-in slide-in-from-bottom-4 duration-300">
                                                        <h3 class="text-[10px] font-bold uppercase tracking-wider text-zinc-400 mb-3 flex items-center justify-between px-1">
                                                            <span>Warteschlange ({scanQueue.length})</span>
                                                            <button onclick={() => scanQueue = []} class="hover:text-red-500 transition-colors cursor-pointer">Clear All</button>
                                                        </h3>
                                                        <div class="flex flex-col gap-2 max-h-48 overflow-y-auto no-scrollbar">
                                                            {#each scanQueue as path}
                                                                <div class="flex items-center justify-between bg-white dark:bg-[#18181b] border border-zinc-200 dark:border-zinc-800/80 rounded-lg px-3 py-2.5 shadow-sm">
                                                                    <div class="flex items-center gap-3 overflow-hidden">
                                                                        <Folder size={14} class="text-zinc-400 shrink-0" />
                                                                        <span class="text-xs font-bold text-zinc-700 dark:text-zinc-300 truncate" title={path}>{path.split(/[/\\]/).pop()}</span>
                                                                    </div>
                                                                    <button onclick={() => scanQueue = scanQueue.filter(p => p !== path)} class="text-zinc-400 hover:text-red-500 shrink-0 ml-2 transition-colors cursor-pointer">
                                                                        <X size={14} />
                                                                    </button>
                                                                </div>
                                                            {/each}
                                                        </div>
                                                    </div>
                                                {/if}
                                            </div>
                                        </div>
                                    {/if}
                                {:else}
                                    <div class="divide-y divide-zinc-100 dark:divide-zinc-800/50 mb-8 transition-opacity duration-200 {isLoading ? 'opacity-40 pointer-events-none' : 'opacity-100'}">
                                        {#each samples as sample}
                                            <div id="sample-{sample.id}" class="group grid grid-cols-[20px_40px_32px_minmax(150px,2fr)_minmax(120px,1.5fr)_50px_40px_40px_32px_32px] items-center gap-4 py-2 rounded-md -mx-2 px-2 {selectedId === sample.id ? 'bg-zinc-100 dark:bg-zinc-800/60' : 'hover:bg-zinc-50 dark:hover:bg-zinc-800/20'}">
                                                <div class="flex justify-center"><input type="checkbox" checked={appState.selectedSampleIds.includes(sample.id)} onchange={(e) => toggleSampleSelection(sample.id, e.currentTarget.checked)} class="h-4 w-4 rounded border-zinc-300 bg-zinc-100 cursor-pointer accent-zinc-900 dark:accent-zinc-100"></div>
                                                <div
                                                        use:nativeDrag={sample}
                                                        class="h-10 w-10 flex items-center justify-center rounded-md bg-zinc-200/50 text-zinc-400 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700/50 overflow-hidden shrink-0 cursor-grab active:cursor-grabbing"
                                                >
                                                    {#if sample.cover_path}
                                                        <img
                                                                src={convertFileSrc(sample.cover_path)}
                                                                alt="Cover"
                                                                class="h-full w-full object-cover pointer-events-none"
                                                                loading="lazy"
                                                        />
                                                    {:else}
                                                        <ImageIcon size={20} class="pointer-events-none" />
                                                    {/if}
                                                </div>
                                                <div class="flex justify-center"><button onclick={() => handlePlayRequest(sample)} class="flex h-8 w-8 items-center justify-center rounded-full bg-zinc-900 text-zinc-100 hover:scale-105 dark:bg-zinc-100 dark:text-zinc-900 transition-transform cursor-pointer shadow-sm">{#if playingId === sample.id} <Pause size={14} /> {:else} <Play size={14} class="ml-0.5" /> {/if}</button></div>

                                                <div class="flex flex-col min-w-0 pr-4 cursor-pointer" role="button" tabindex="0" onclick={() => { selectedId = sample.id; }} onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') selectedId = sample.id; }}>
                                                    <span
                                                            use:nativeDrag={sample}
                                                            class="truncate text-sm font-semibold hover:underline cursor-grab active:cursor-grabbing select-none"
                                                            title={sample.original_path}
                                                    >
                                                        {sample.filename}
                                                    </span>
                                                    <div class="flex flex-wrap gap-1.5 mt-1 h-4 overflow-hidden">
                                                        {#each parseTags(sample.tags) as tag}
                                                            <span class="rounded px-1.5 py-[1px] text-[9px] font-bold uppercase tracking-wider {tag.category === 'Format' ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400' : tag.category === 'Drums' || tag.category === 'Percussion' ? 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400' : tag.category === 'Genre' ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400' : 'bg-zinc-200/60 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400'}">{tag.value}</span>
                                                        {/each}
                                                        {#if parseTags(sample.tags).length === 0} <span class="rounded bg-zinc-200/60 px-1.5 py-[1px] text-[9px] font-bold uppercase tracking-wider text-zinc-400 dark:bg-zinc-800">AUDIO</span> {/if}
                                                    </div>
                                                </div>

                                                <div
                                                        use:nativeDrag={sample}
                                                        class="flex items-center gap-[2px] h-8 overflow-hidden opacity-60 group-hover:opacity-100 transition-opacity cursor-grab active:cursor-grabbing"
                                                >
                                                    {#each parseWaveform(sample.waveform_data) as barHeight, i}
                                                        <div class="w-[3px] rounded-full pointer-events-none {playingId === sample.id && (i / 40) <= appState.playbackProgress ? 'bg-emerald-500' : 'bg-zinc-300 dark:bg-zinc-700'}" style="height: {barHeight}%;"></div>
                                                    {/each}
                                                </div>
                                                <div class="text-right text-xs font-medium text-zinc-500 tabular-nums">{formatDuration(sample.duration_ms)}</div>
                                                <div class="text-center text-xs font-semibold text-zinc-700 dark:text-zinc-300">{sample.key_signature || "--"}</div>
                                                <div class="text-center text-xs font-semibold text-zinc-700 dark:text-zinc-300">{sample.bpm ? Math.round(sample.bpm) : "--"}</div>
                                                <div class="flex justify-center"><button onclick={(e) => toggleLike(sample, e)} class="transition-colors cursor-pointer group-hover:opacity-100 {selectedId === sample.id || sample.is_liked ? 'opacity-100' : 'opacity-0'} {sample.is_liked ? 'text-red-500 hover:text-red-600' : 'text-zinc-400 hover:text-red-500'}"><Heart size={16} class={sample.is_liked ? 'fill-red-500' : ''} /></button></div>
                                                <div class="relative flex justify-center">
                                                    <button onclick={(e) => { e.stopPropagation(); openContextMenuId = openContextMenuId === sample.id ? null : sample.id; }} class="text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer group-hover:opacity-100 {selectedId === sample.id || openContextMenuId === sample.id ? 'opacity-100' : 'opacity-0'}"><EllipsisVertical size={16} /></button>
                                                    {#if openContextMenuId === sample.id}
                                                        <div onclick={(e) => e.stopPropagation()} class="absolute right-full top-0 mr-2 w-40 flex-col rounded-lg border border-zinc-200 bg-white p-1 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                                                            <button onclick={() => { openSampler(sample); openContextMenuId = null; }} class="w-full text-left rounded-md px-3 py-2 text-xs font-medium text-emerald-600 hover:bg-emerald-50 dark:text-emerald-400 dark:hover:bg-emerald-900/30 cursor-pointer transition-colors">Open in Sampler</button>
                                                            <div class="my-0.5 border-t border-zinc-200 dark:border-zinc-800/50"></div>
                                                            <button onclick={() => openEditModal(sample)} class="w-full text-left rounded-md px-3 py-2 text-xs font-medium text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800 cursor-pointer transition-colors">Edit Metadata</button>
                                                            <div class="my-0.5 border-t border-zinc-200 dark:border-zinc-800/50"></div>
                                                            <button onclick={() => { invoke('reveal_in_finder', { path: sample.original_path }); openContextMenuId = null; }} class="w-full text-left rounded-md px-3 py-2 text-xs font-medium text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800 cursor-pointer transition-colors">Reveal in Finder</button>
                                                        </div>
                                                    {/if}
                                                </div>
                                            </div>
                                        {/each}
                                    </div>
                                {/if}
                            </div>
                        </div>
                    </div>

                    {#if totalPages > 1}
                        <div class="w-full">
                            <div class="flex items-center justify-center pb-8 pt-4">
                                <div class="flex items-center gap-1">
                                    <button onclick={prevPage} disabled={currentPage === 1} class="flex items-center justify-center h-8 w-8 rounded text-zinc-600 hover:bg-zinc-100 disabled:opacity-30 disabled:hover:bg-transparent dark:text-zinc-400 dark:hover:bg-zinc-800 transition-colors cursor-pointer mr-2"><ChevronLeft size={18} /></button>
                                    {#each visiblePages as pageNum} <button onclick={() => goToPage(pageNum)} class="flex items-center justify-center h-8 w-8 rounded text-sm font-medium transition-colors cursor-pointer {pageNum === currentPage ? 'bg-zinc-900 text-white dark:bg-zinc-100 dark:text-zinc-900' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800'}">{pageNum}</button> {/each}
                                    <button onclick={nextPage} disabled={currentPage === totalPages} class="flex items-center justify-center h-8 w-8 rounded text-zinc-600 hover:bg-zinc-100 disabled:opacity-30 disabled:hover:bg-transparent dark:text-zinc-400 dark:hover:bg-zinc-800 transition-colors cursor-pointer ml-2"><ChevronRight size={18} /></button>
                                </div>
                            </div>
                        </div>
                    {/if}

                </div>
            {/if}
        </div>

        <div class="absolute right-0 top-0 bottom-0 w-72 bg-white/95 backdrop-blur-xl border-l border-zinc-200 dark:border-zinc-800/60 dark:bg-[#18181b]/95 shadow-2xl transition-transform duration-300 flex flex-col z-50 {appState.selectedSampleIds.length > 0 ? 'translate-x-0' : 'translate-x-full'}">
            <div class="flex items-center justify-between px-5 py-4 border-b border-zinc-200 dark:border-zinc-800/60">
                <span class="font-bold text-sm">{appState.selectedSampleIds.length} Items Selected</span>
                <button onclick={() => appState.selectedSampleIds = []} class="text-zinc-400 hover:text-zinc-900 dark:hover:text-white cursor-pointer"><X size={16} /></button>
            </div>
            <div class="flex-1 overflow-y-auto p-3 space-y-1 no-scrollbar">

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

                <div class="my-4 border-t border-zinc-200 dark:border-zinc-800/50"></div>
                <div class="flex items-center justify-between px-2">
                    <span class="text-[10px] font-bold uppercase tracking-wider text-zinc-400">Shared Tags</span>
                    {#if isBulkTagging}<RefreshCw size={12} class="animate-spin text-zinc-400" />{/if}
                </div>

                <div class="px-2 mt-2">
                    <div class="flex flex-wrap gap-1.5 mb-4 min-h-[24px]">
                        {#each commonTags as tag}
                            <div class="group relative flex items-center justify-center h-6 rounded-full border border-zinc-200 bg-zinc-50 px-2.5 text-[10px] font-semibold text-zinc-600 transition-all hover:pr-6 dark:border-zinc-800 dark:bg-[#18181b] dark:text-zinc-300 cursor-default overflow-hidden shadow-sm">
                                <span>{tag.value}</span>
                                <button onclick={() => handleBulkRemoveTag(tag.category, tag.value)} class="absolute right-1 opacity-0 group-hover:opacity-100 flex h-4 w-4 items-center justify-center rounded-full bg-zinc-200 text-zinc-600 hover:bg-red-500 hover:text-white dark:bg-zinc-700 dark:text-zinc-300 dark:hover:bg-red-600 transition-all cursor-pointer"><X size={10} /></button>
                            </div>
                        {/each}
                        {#if commonTags.length === 0}
                            <span class="text-[10px] text-zinc-500 italic flex items-center h-6">No tags shared by all selected.</span>
                        {/if}
                    </div>

                    <div class="relative bulk-tag-dropdown-container">
                        <button onclick={(e) => { e.stopPropagation(); isBulkTagDropdownOpen = !isBulkTagDropdownOpen; }} class="flex w-full items-center justify-between rounded-md border border-zinc-300 bg-white px-3 py-2 text-xs font-semibold text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-300 dark:hover:bg-zinc-800 transition-colors cursor-pointer shadow-sm">
                            <span class="flex items-center gap-2"><Plus size={14} /> Add Tag to All</span>
                            <ChevronDown size={14} class="opacity-50" />
                        </button>

                        {#if isBulkTagDropdownOpen}
                            <div onclick={(e) => e.stopPropagation()} class="absolute left-0 top-full mt-2 w-full flex-col rounded-lg border border-zinc-200 bg-white p-2 shadow-2xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                                <input type="text" bind:value={bulkTagSearchQuery} placeholder="Search tags..." class="w-full rounded-md border border-zinc-200 bg-zinc-50 px-2 py-1.5 text-xs focus:border-emerald-500 focus:outline-none dark:border-zinc-700 dark:bg-zinc-900 dark:text-white transition-colors mb-2" autofocus />

                                {#if bulkTagSearchQuery.trim() !== '' && !allAvailableTags.some(t => t.value.toLowerCase() === bulkTagSearchQuery.trim().toLowerCase())}
                                    <button onclick={createNewBulkTag} class="w-full mb-2 flex items-center justify-center gap-1.5 rounded-md bg-emerald-50 text-emerald-600 px-2 py-1.5 text-xs font-bold hover:bg-emerald-100 dark:bg-emerald-900/20 dark:text-emerald-400 dark:hover:bg-emerald-900/40 transition-colors cursor-pointer shadow-sm">
                                        <Plus size={12} /> Create global Tag "{bulkTagSearchQuery}"
                                    </button>
                                {/if}

                                <div class="max-h-48 overflow-y-auto no-scrollbar flex flex-col gap-0.5 border-t border-zinc-100 dark:border-zinc-800/50 pt-2">
                                    {#each filteredBulkTags as tag}
                                        <button
                                                onclick={() => handleBulkAddTag(tag.category, tag.value)}
                                                class="w-full text-left flex items-center justify-between rounded-md px-2 py-1.5 text-xs font-medium text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800 cursor-pointer transition-colors"
                                        >
                                            <span class="truncate pr-2">{tag.value}</span>
                                            <span class="text-[8px] uppercase text-zinc-400 font-bold tracking-wider shrink-0">{tag.category}</span>
                                        </button>
                                    {/each}
                                    {#if filteredBulkTags.length === 0 && bulkTagSearchQuery.trim() === ''}
                                        <span class="text-xs text-zinc-500 italic p-2 text-center">No tags found.</span>
                                    {/if}
                                </div>
                            </div>
                        {/if}
                    </div>
                </div>
            </div>
        </div>

    </div>
{:else if appState.currentView === 'projects'}
    <div class="flex h-full items-center justify-center text-zinc-500"><h2 class="text-2xl font-bold">Musik Projekte (Coming Soon)</h2></div>
{:else if appState.currentView === 'editor'}
    <div class="flex h-full w-full flex-col overflow-y-auto px-10 py-8">
    </div>
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

                <h3 class="text-xs font-bold uppercase tracking-wider text-zinc-400 border-b border-zinc-200 dark:border-zinc-800/50 pb-2 mb-6 mt-10">Playback</h3>

                <div class="flex items-center justify-between">
                    <div>
                        <label class="text-sm font-medium text-zinc-900 dark:text-zinc-100 block">Auto-Play Selection</label>
                        <p class="text-xs text-zinc-500 mt-1">Automatically play sounds when navigating the list with arrow keys.</p>
                    </div>
                    <label class="relative inline-flex items-center cursor-pointer">
                        <input type="checkbox" bind:checked={appState.autoPlayEnabled} class="sr-only peer">
                        <div class="w-11 h-6 bg-zinc-200 peer-focus:outline-none rounded-full peer dark:bg-zinc-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-zinc-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-zinc-600 peer-checked:bg-emerald-500"></div>
                    </label>
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
            {:else if appState.activeSettingsTab === 'tags'}
                <div>
                    <h3 class="text-xs font-bold uppercase tracking-wider text-zinc-400 border-b border-zinc-200 dark:border-zinc-800/50 pb-2 mb-6">Tag Management</h3>

                    <div class="space-y-2">
                        {#each allAvailableTags.filter(t => t.category === 'User') as tag}
                            <div class="flex items-center justify-between rounded-md border border-zinc-200 bg-white p-3 shadow-sm dark:border-zinc-800 dark:bg-zinc-900/50">
                                <div class="flex flex-col overflow-hidden pr-4">
                                    <span class="truncate text-sm font-medium text-zinc-700 dark:text-zinc-300">{tag.value}</span>
                                    <span class="text-[10px] uppercase text-zinc-500 font-bold tracking-wider mt-0.5">Global User Tag</span>
                                </div>
                                <button onclick={(e) => handleDeleteUserTag(tag.value, e)} class="shrink-0 rounded-md border border-red-200 bg-red-50 px-3 py-1.5 text-xs font-medium text-red-700 hover:bg-red-100 transition-colors dark:border-red-900/30 dark:bg-red-900/10 dark:text-red-400 dark:hover:bg-red-900/20 cursor-pointer shadow-sm">Delete</button>
                            </div>
                        {/each}

                        {#if allAvailableTags.filter(t => t.category === 'User').length === 0}
                            <div class="flex h-20 items-center justify-center rounded-md border border-dashed border-zinc-200 text-sm text-zinc-500 dark:border-zinc-800">No custom tags created yet.</div>
                        {/if}
                    </div>

                    <p class="mt-4 text-xs text-zinc-500">Deleting a tag here will automatically remove it from all samples in your entire library. This action cannot be undone.</p>
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
                                                    class="w-full text-left flex items-center justify-between rounded-md px-2 py-1.5 text-xs font-medium text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800 cursor-pointer transition-colors"
                                            >
                                                <span class="truncate pr-2">{tag.value}</span>
                                                <span class="text-[8px] uppercase text-zinc-400 font-bold tracking-wider shrink-0">{tag.category}</span>
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

{#if isSamplerOpen && samplerSample}
    <div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/60 backdrop-blur-md animate-in fade-in duration-200 p-8">
        <div class="w-full max-w-5xl bg-white dark:bg-[#18181b] border border-zinc-200 dark:border-zinc-800 rounded-2xl shadow-2xl flex flex-col overflow-hidden">

            <div class="flex items-center justify-between px-6 py-4 border-b border-zinc-100 dark:border-zinc-800/50 bg-zinc-50/50 dark:bg-zinc-900/50">
                <div class="flex items-center gap-4">
                    <div class="h-12 w-12 flex items-center justify-center rounded-md bg-zinc-200 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700 overflow-hidden shrink-0 shadow-sm">
                        {#if samplerSample.cover_path}
                            <img src={convertFileSrc(samplerSample.cover_path)} alt="Cover" class="h-full w-full object-cover" />
                        {:else}
                            <ImageIcon size={20} class="text-zinc-400" />
                        {/if}
                    </div>
                    <div>
                        <h2 class="text-lg font-bold text-zinc-900 dark:text-white leading-none">{samplerSample.filename}</h2>
                        <div class="flex items-center gap-2 mt-2 text-[11px] font-semibold text-zinc-500 uppercase tracking-wider">
                            <span class="bg-zinc-200/60 dark:bg-zinc-800 px-1.5 py-[2px] rounded text-zinc-700 dark:text-zinc-300">{samplerSample.extension}</span>
                            <span>{formatDuration(samplerSample.duration_ms)}</span>
                            {#if samplerSample.bpm}<span>• {Math.round(samplerSample.bpm)} BPM</span>{/if}
                            {#if samplerSample.key_signature}<span>• {samplerSample.key_signature}</span>{/if}
                        </div>
                    </div>
                </div>
                <button onclick={closeSampler} class="h-8 w-8 flex items-center justify-center rounded-full text-zinc-400 hover:bg-zinc-200 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-white transition-colors cursor-pointer">
                    <X size={20} />
                </button>
            </div>

            <div class="p-6 pb-8">
                <div class="flex justify-between mb-3 text-[11px] font-bold uppercase tracking-wider text-zinc-400 tabular-nums px-1">
                    <span>Start: {formatDuration(samplerSample.duration_ms * trimStartPct)}</span>
                    <span class="text-emerald-600 dark:text-emerald-400 bg-emerald-50 dark:bg-emerald-900/20 px-2 py-0.5 rounded border border-emerald-200 dark:border-emerald-800/50">
                        Selection Length: {formatDuration(samplerSample.duration_ms * (trimEndPct - trimStartPct))}
                    </span>
                    <span>End: {formatDuration(samplerSample.duration_ms * trimEndPct)}</span>
                </div>

                <div
                        bind:this={editorScrollContainer}
                        use:editorWheelZoom
                        class="w-full h-48 bg-zinc-100 dark:bg-[#121214] border border-zinc-200 dark:border-zinc-800 rounded-xl {editorZoomLevel > 1.0 ? 'overflow-x-auto' : 'overflow-hidden'} overflow-y-hidden shadow-inner relative no-scrollbar"
                >
                    <div
                            bind:this={editorWaveformContainer}
                            class="relative h-full select-none origin-left"
                            style="width: {editorZoomLevel * 100}%"
                    >
                        {#if !isSliceReady}
                            <div class="absolute inset-0 z-20 cursor-crosshair" onmousedown={handleWaveformClick} role="button" tabindex="0"></div>
                        {/if}

                        <div class="absolute inset-0 flex items-center justify-between gap-[1px] opacity-20 pointer-events-none">
                            {#each parseWaveform(samplerSample.waveform_data, Math.floor(300 * editorZoomLevel)) as barHeight}
                                <div class="w-full rounded-full bg-zinc-500" style="height: {barHeight}%;"></div>
                            {/each}
                        </div>

                        <div
                                class="absolute inset-0 flex items-center justify-between gap-[1px] pointer-events-none"
                                style="clip-path: polygon({trimStartPct * 100}% 0, {trimEndPct * 100}% 0, {trimEndPct * 100}% 100%, {trimStartPct * 100}% 100%);"
                        >
                            {#each parseWaveform(samplerSample.waveform_data, Math.floor(300 * editorZoomLevel)) as barHeight}
                                <div class="w-full rounded-full {isSliceReady ? 'bg-emerald-400' : 'bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.4)]'} transition-colors" style="height: {barHeight}%;"></div>
                            {/each}
                        </div>

                        <div
                                class="absolute top-0 bottom-0 {isSliceReady ? 'bg-emerald-500/10' : 'bg-transparent'} transition-colors duration-300 pointer-events-none"
                                style="left: {trimStartPct * 100}%; right: {(1 - trimEndPct) * 100}%"
                        >
                            <div class="absolute -top-3 left-1/2 -translate-x-1/2 opacity-0 group-hover:opacity-100 transition-opacity mt-6 z-30 pointer-events-none">
                                <div class="bg-zinc-900 dark:bg-black text-white text-[10px] font-bold px-3 py-1.5 rounded-full shadow-xl flex items-center gap-1.5 border border-zinc-700/50">
                                    {#if isSlicing}
                                        <RefreshCw size={12} class="animate-spin text-emerald-400" /> <span>Slicing...</span>
                                    {:else}
                                        <Download size={12} class="text-emerald-400" /> <span>Drag to DAW</span>
                                    {/if}
                                </div>
                            </div>

                            {#if appState.isPlaying && isSamplerOpen}
                                <div class="absolute top-0 bottom-0 w-0.5 bg-white shadow-[0_0_10px_rgba(255,255,255,1)] z-50 pointer-events-none" style="left: {appState.playbackProgress * 100}%"></div>
                            {/if}

                            {#if isSliceReady}
                                <div
                                        use:nativeSliceDrag
                                        class="absolute inset-0 flex items-center justify-center opacity-0 hover:opacity-100 transition-opacity bg-emerald-900/40 cursor-grab active:cursor-grabbing backdrop-blur-sm z-30 pointer-events-auto"
                                >
                                    <span class="text-white font-bold tracking-wider uppercase text-sm px-4 py-2 bg-black/80 shadow-xl rounded-md flex items-center gap-2"><Download size={16} /> Drag to DAW</span>
                                </div>
                            {:else}
                                <div class="absolute left-0 top-0 bottom-0 w-6 cursor-ew-resize flex flex-col items-start group/handle z-40 pointer-events-auto" onmousedown={(e) => { isDraggingHandle = 'start'; e.stopPropagation(); }}>
                                    <div class="bg-emerald-600 group-hover/handle:bg-emerald-400 text-white text-[9px] font-black px-1.5 py-0.5 rounded-br-md shadow-md transition-colors">S</div>
                                    <div class="w-[2px] h-full bg-emerald-600/80 group-hover/handle:bg-emerald-400 shadow-sm transition-colors -mt-[1px]"></div>
                                </div>

                                <div class="absolute right-0 top-0 bottom-0 w-6 cursor-ew-resize flex flex-col items-end group/handle z-40 pointer-events-auto" onmousedown={(e) => { isDraggingHandle = 'end'; e.stopPropagation(); }}>
                                    <div class="bg-emerald-600 group-hover/handle:bg-emerald-400 text-white text-[9px] font-black px-1.5 py-0.5 rounded-bl-md shadow-md transition-colors">E</div>
                                    <div class="w-[2px] h-full bg-emerald-600/80 group-hover/handle:bg-emerald-400 shadow-sm transition-colors -mt-[1px]"></div>
                                </div>
                            {/if}
                        </div>
                    </div>
                </div>

                <div class="mt-5 flex items-center justify-between">
                    <div class="flex items-center gap-4">
                        <button onclick={() => isLooping = !isLooping} class="flex items-center gap-2 px-3 py-2 rounded-md text-[11px] font-bold uppercase tracking-wider transition-colors {isLooping ? 'bg-emerald-500 text-white shadow-sm' : 'bg-zinc-100 text-zinc-600 hover:bg-zinc-200 dark:bg-zinc-800/50 dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-white'} cursor-pointer">
                            <Repeat size={14} /> Loop
                        </button>

                        <div class="flex items-center rounded-md border border-zinc-200 dark:border-zinc-800/60 bg-white dark:bg-[#18181b] shadow-sm">
                            <button onclick={handleZoomOut} disabled={editorZoomLevel <= 1.0} class="flex items-center justify-center h-8 w-8 text-zinc-500 hover:text-zinc-900 hover:bg-zinc-50 dark:hover:bg-zinc-800 dark:hover:text-zinc-100 disabled:opacity-30 transition-colors rounded-l-md cursor-pointer"><ZoomOut size={14} /></button>
                            <div class="w-px h-4 bg-zinc-200 dark:bg-zinc-800"></div>
                            <span class="text-[10px] font-bold text-zinc-500 w-12 text-center tabular-nums cursor-default select-none" title="Cmd/Ctrl + Mousewheel to zoom">
                                {Math.round(editorZoomLevel * 100)}%
                            </span>
                            <div class="w-px h-4 bg-zinc-200 dark:bg-zinc-800"></div>
                            <button onclick={handleZoomIn} disabled={editorZoomLevel >= 20.0} class="flex items-center justify-center h-8 w-8 text-zinc-500 hover:text-zinc-900 hover:bg-zinc-50 dark:hover:bg-zinc-800 dark:hover:text-zinc-100 disabled:opacity-30 transition-colors rounded-r-md cursor-pointer"><ZoomIn size={14} /></button>
                        </div>
                    </div>

                    <div class="flex items-center gap-3">
                        <button onclick={playSlicePreview} class="flex items-center gap-2 px-4 py-2 rounded-md text-xs font-bold uppercase tracking-wider transition-colors {appState.isPlaying ? 'bg-zinc-200 text-zinc-900 dark:bg-zinc-800 dark:text-white' : 'bg-zinc-100 text-zinc-600 hover:bg-zinc-200 dark:bg-zinc-800/50 dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-white'} cursor-pointer">
                            {#if appState.isPlaying} <Pause size={14}/> Stop {:else} <Play size={14}/> Preview (Space) {/if}
                        </button>

                        {#if !isSliceReady}
                            <button onclick={confirmSlice} disabled={isSlicing} class="flex items-center gap-2 px-5 py-2 rounded-md text-xs font-bold uppercase tracking-wider bg-emerald-500 text-white hover:bg-emerald-600 transition-colors shadow-sm disabled:opacity-50 cursor-pointer">
                                {#if isSlicing} <RefreshCw size={14} class="animate-spin"/> Rendering... {:else} Confirm Selection {/if}
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