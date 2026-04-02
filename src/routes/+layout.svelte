<script lang="ts">
    import '../app.css';
    import { onMount } from 'svelte';
    import { appState } from '$lib/store.svelte';
    import { getCurrentWindow } from '@tauri-apps/api/window'; // NEU: Direkte Tauri Window API
    import { invoke, convertFileSrc } from '@tauri-apps/api/core';
    import {
        Compass, Folder, Heart, Plus, Search, Play, Pause,
        SkipBack, SkipForward, Volume2, Sun, Moon, Library, Settings,
        Music, Type, Image as ImageIcon, FolderOpen
    } from 'lucide-svelte';

    let { children } = $props();

    // --- ENTERPRISE THEME MANAGEMENT ---
    let systemMedia: MediaQueryList;

    // --- ENTERPRISE WINDOW DRAGGING & MAXIMIZE ---
    function dragRegion(node: HTMLElement) {
        let lastClickTime = 0;

        const handleMouseDown = async (e: MouseEvent) => {
            const target = e.target as HTMLElement;

            // Interaktive Elemente schützen
            if (target.tagName === 'INPUT' || target.tagName === 'BUTTON' || target.closest('button')) {
                return;
            }

            if (e.buttons === 1) { // Nur Linksklick
                const currentTime = Date.now();
                const timeDiff = currentTime - lastClickTime;
                lastClickTime = currentTime;

                // Doppelklick-Erkennung (400ms Threshold)
                if (timeDiff < 400) {
                    try {
                        await getCurrentWindow().toggleMaximize();
                        return; // Breche hier ab, damit wir nicht ins Dragging wechseln
                    } catch(err) {
                        console.error("Failed to toggle maximize:", err);
                    }
                }

                // Einfacher Klick: Normales Dragging
                try {
                    getCurrentWindow().startDragging();
                } catch(err) {
                    console.error("Failed to drag window:", err);
                }
            }
        };

        node.addEventListener('mousedown', handleMouseDown);

        return {
            destroy() {
                node.removeEventListener('mousedown', handleMouseDown);
            }
        };
    }

    onMount(async() => {
        const savedTheme = localStorage.getItem('samplevault-theme') as 'light' | 'dark' | 'system' | null;
        if (savedTheme) {
            appState.themePreference = savedTheme;
        }

        // ENTERPRISE FIX: Gespeichertes Volume synchron laden und validieren
        const savedVolume = localStorage.getItem('samplevault-volume');
        if (savedVolume !== null) {
            const parsedVolume = parseFloat(savedVolume);
            // Sicherheits-Check: Nur übernehmen, wenn es eine valide Zahl zwischen 0 und 1 ist
            if (!isNaN(parsedVolume) && parsedVolume >= 0 && parsedVolume <= 1) {
                appState.globalVolume = parsedVolume;
            }
        }

        systemMedia = window.matchMedia('(prefers-color-scheme: dark)');
        systemMedia.addEventListener('change', applyTheme);

        if (typeof window !== 'undefined') {
            screenWidth = window.screen.availWidth;
            windowWidth = window.innerWidth;
        }

        applyTheme();
        await loadCollections();
    });

    async function loadCollections() {
        try {
            appState.collections = await invoke('get_collections');
        } catch (e) { console.error(e); }
    }

    let newCollectionName = $state('');

    // --- RESPONSIVE TAB BAR STATE ---
    let windowWidth = $state(0);
    let screenWidth = $state(0);

    // Berechnet live, ob das Fenster größer als 75% der Bildschirmbreite ist
    let showTabLabels = $derived(screenWidth > 0 ? windowWidth > (screenWidth * 0.75) : true);

    async function submitCollection() {
        if (!newCollectionName || newCollectionName.trim() === '') return;
        try {
            await invoke('create_collection', { name: newCollectionName.trim() });
            await loadCollections();
            appState.isCreateCollectionModalOpen = false;
            newCollectionName = '';
        } catch (e) {
            alert("Error: Name might already exist.");
        }
    }

    $effect(() => {
        if (appState.themePreference) {
            applyTheme();
        }
    });

    $effect(() => {
        if (typeof window !== 'undefined' && appState.globalVolume !== undefined) {
            localStorage.setItem('samplevault-volume', appState.globalVolume.toString());
        }
    });

    function applyTheme() {
        if (typeof window === 'undefined') return;

        const prefersDark = systemMedia ? systemMedia.matches : window.matchMedia('(prefers-color-scheme: dark)').matches;

        if (appState.themePreference === 'dark' || (appState.themePreference === 'system' && prefersDark)) {
            document.documentElement.classList.add('dark');
            appState.isDarkMode = true;
        } else {
            document.documentElement.classList.remove('dark');
            appState.isDarkMode = false;
        }
    }

    function toggleTheme() {
        appState.themePreference = appState.isDarkMode ? 'light' : 'dark';
        localStorage.setItem('samplevault-theme', appState.themePreference);
    }

    // --- SIDEBAR NAVIGATION ---

    // 1. Zurück zur globalen Haupt-Bibliothek
    function switchToMainLibrary() {
        if (appState.currentView === 'sounds' && !appState.filters.onlyLiked && appState.filters.collectionId === null) return;
        appState.currentView = 'sounds';
        appState.activeSoundsTab = 'samples'; // NEU
        appState.filters.onlyLiked = false;
        appState.filters.collectionId = null;
        if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent('trigger-sample-reload'));
    }

    // 2. Zum Liked-Ordner wechseln
    function switchToLiked(enabled: boolean) {
        if (appState.filters.onlyLiked === enabled && appState.currentView === 'sounds' && appState.filters.collectionId === null) return;
        appState.currentView = 'sounds';
        appState.activeSoundsTab = 'collections'; // NEU (Liked gehört konzeptionell zu Collections)
        appState.filters.onlyLiked = enabled;
        appState.filters.collectionId = null;
        if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent('trigger-sample-reload'));
    }

    // 3. Zu einer Collection wechseln
    function switchToCollection(id: number | null) {
        if (appState.filters.collectionId === id && appState.currentView === 'sounds') return;
        appState.currentView = 'sounds';
        appState.activeSoundsTab = 'collections'; // NEU
        appState.filters.collectionId = id;
        appState.filters.onlyLiked = false;
        if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent('trigger-sample-reload'));
    }
</script>

<svelte:window bind:innerWidth={windowWidth} />

<div class="absolute inset-0 flex flex-col overflow-hidden bg-white text-zinc-900 dark:bg-[#121212] dark:text-zinc-100 font-sans antialiased" style="will-change: transform, width, height;">

    <div class="flex flex-1 overflow-hidden pb-16">

        <aside class="flex w-64 flex-col border-r border-zinc-200 bg-zinc-50 dark:border-zinc-800/60 dark:bg-[#18181b]">

            <div use:dragRegion class="flex h-14 items-center pl-25 pr-6 border-b border-zinc-200 dark:border-zinc-800/60 shrink-0 select-none">
                <span class="font-bold tracking-tight text-lg">SampleVault</span>
            </div>

            <nav class="flex-1 overflow-y-auto px-3 py-4 space-y-6">
                {#if appState.currentView === 'sounds'}
                    <div class="space-y-1">
                        <button
                                onclick={switchToMainLibrary}
                                class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors cursor-pointer {!appState.filters.onlyLiked && appState.filters.collectionId === null ? 'bg-zinc-200/50 dark:bg-zinc-800/50 text-zinc-900 dark:text-zinc-50' : 'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100'}"
                        >
                            <Library size={18} /> Sounds
                        </button>

                        <button class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100 transition-colors cursor-pointer">
                            <Compass size={18} /> Discover
                        </button>
                    </div>

                    <div>
                        <div class="mb-2 flex items-center justify-between px-3 text-xs font-semibold uppercase tracking-wider text-zinc-500">
                            <span>Collections</span>
                            <button onclick={() => appState.isCreateCollectionModalOpen = true} class="hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer">
                                <Plus size={14} />
                            </button>
                        </div>
                        <div class="space-y-1">
                            <button
                                    onclick={() => { appState.filters.collectionId = null; switchToLiked(true); }}
                                    class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors cursor-pointer {appState.filters.onlyLiked ? 'bg-zinc-200/50 dark:bg-zinc-800/50 text-zinc-900 dark:text-zinc-50' : 'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100'}"
                            >
                                <Heart size={18} class={appState.filters.onlyLiked ? 'fill-red-500 text-red-500' : ''} /> Likes
                            </button>

                            {#each appState.collections as collection}
                                <button
                                        onclick={() => switchToCollection(collection.id)}
                                        class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors cursor-pointer {appState.filters.collectionId === collection.id ? 'bg-zinc-200/50 dark:bg-zinc-800/50 text-zinc-900 dark:text-zinc-50' : 'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100'}"
                                >
                                    <Folder size={18} class={appState.filters.collectionId === collection.id ? 'fill-zinc-900 dark:fill-zinc-100' : ''} /> {collection.name}
                                </button>
                            {/each}
                        </div>
                    </div>

                {:else if appState.currentView === 'projects'}
                    <div class="mb-2 px-3 text-xs font-semibold uppercase tracking-wider text-zinc-500">Workspace</div>
                    <div class="px-3 text-sm text-zinc-400 italic">Project tools will appear here...</div>

                {:else if appState.currentView === 'editor'}
                    <div class="mb-2 px-3 text-xs font-semibold uppercase tracking-wider text-zinc-500">Batch Renamer</div>
                    <div class="px-3 text-sm text-zinc-400 italic">Metadata inputs will appear here...</div>

                {:else if appState.currentView === 'settings'}
                    <div class="mb-2 px-3 text-xs font-semibold uppercase tracking-wider text-zinc-500">Preferences</div>
                    <div class="space-y-1">
                        <button onclick={() => appState.activeSettingsTab = 'general'} class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors cursor-pointer {appState.activeSettingsTab === 'general' ? 'bg-zinc-200/50 text-zinc-900 dark:bg-zinc-800/50 dark:text-zinc-50' : 'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100'}">
                            General
                        </button>
                        <button onclick={() => appState.activeSettingsTab = 'library'} class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors cursor-pointer {appState.activeSettingsTab === 'library' ? 'bg-zinc-200/50 text-zinc-900 dark:bg-zinc-800/50 dark:text-zinc-50' : 'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100'}">
                            Library
                        </button>
                        <button onclick={() => appState.activeSettingsTab = 'audio'} class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors cursor-pointer {appState.activeSettingsTab === 'audio' ? 'bg-zinc-200/50 text-zinc-900 dark:bg-zinc-800/50 dark:text-zinc-50' : 'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100'}">
                            Audio
                        </button>
                    </div>
                {/if}
            </nav>
        </aside>

        <div class="flex flex-1 flex-col relative min-w-0">

            <header use:dragRegion class="sticky top-0 z-10 flex h-14 items-center justify-between border-b border-zinc-200 bg-white/80 px-6 backdrop-blur-md dark:border-zinc-800/60 dark:bg-[#121212]/80 shrink-0 select-none">

                <div class="flex flex-1 items-center relative">
                    <Search size={16} class="absolute left-3 text-zinc-400" />
                    <input
                            data-tauri-drag-region="false"
                            type="text"
                            bind:value={appState.globalSearchQuery}
                            placeholder={appState.currentView === 'sounds' ? "Search your library..." : appState.currentView === 'projects' ? "Search projects..." : "Search..."}
                            class="h-9 w-full max-w-sm rounded-md border border-zinc-200 bg-zinc-50 pl-9 pr-4 text-sm outline-none focus:border-zinc-300 dark:border-zinc-800 dark:bg-zinc-900 dark:focus:border-zinc-700 transition-colors"
                    >
                </div>

                <div class="flex items-center gap-6">
                    <div data-tauri-drag-region="false" class="flex rounded-lg bg-zinc-100 p-1 dark:bg-zinc-800/50 border border-zinc-200 dark:border-zinc-700/50">
                        <button onclick={() => appState.currentView = 'sounds'} class="flex items-center justify-center gap-2 rounded-md {showTabLabels ? 'px-4' : 'w-8'} py-1.5 text-xs font-semibold transition-all cursor-pointer {appState.currentView === 'sounds' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">
                            <Library size={14} class="shrink-0" />
                            {#if showTabLabels}<span>Sounds</span>{/if}
                        </button>
                        <button onclick={() => appState.currentView = 'projects'} class="flex items-center justify-center gap-2 rounded-md {showTabLabels ? 'px-4' : 'w-8'} py-1.5 text-xs font-semibold transition-all cursor-pointer {appState.currentView === 'projects' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">
                            <Music size={14} class="shrink-0" />
                            {#if showTabLabels}<span>Projects</span>{/if}
                        </button>
                        <button onclick={() => appState.currentView = 'editor'} class="flex items-center justify-center gap-2 rounded-md {showTabLabels ? 'px-4' : 'w-8'} py-1.5 text-xs font-semibold transition-all cursor-pointer {appState.currentView === 'editor' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">
                            <Type size={14} class="shrink-0" />
                            {#if showTabLabels}<span>Pack Editor</span>{/if}
                        </button>
                    </div>

                    <div class="h-5 w-px bg-zinc-200 dark:bg-zinc-700 pointer-events-none"></div>

                    <div data-tauri-drag-region="false" class="flex items-center gap-4">
                        <button onclick={toggleTheme} class="text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100 transition-colors cursor-pointer" title="Toggle Theme">
                            {#if appState.isDarkMode} <Sun size={18} /> {:else} <Moon size={18} /> {/if}
                        </button>
                        <button
                                onclick={() => appState.currentView = appState.currentView === 'settings' ? 'sounds' : 'settings'}
                                class="transition-colors cursor-pointer {appState.currentView === 'settings' ? 'text-zinc-900 dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100'}"
                                title="Preferences"
                        >
                            <Settings size={18} />
                        </button>
                    </div>
                </div>
            </header>

            <main class="flex-1 overflow-y-auto">{@render children()}</main>
        </div>
    </div>

    <footer class="absolute bottom-0 left-0 z-50 flex h-16 w-full items-center justify-between border-t border-zinc-200 bg-white/90 px-6 backdrop-blur-xl dark:border-zinc-800/60 dark:bg-[#18181b]/90">

        <div class="absolute top-[0px] left-0 h-[2px] bg-emerald-500 z-50" style="width: {appState.playbackProgress * 100}%"></div>

        <div class="flex items-center gap-4 w-1/4">
            <button onclick={() => appState.cmdPrev++} class="text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer"><SkipBack size={20} /></button>
            <button onclick={() => appState.cmdTogglePlay++} class="flex h-10 w-10 items-center justify-center rounded-full bg-zinc-900 text-white hover:bg-zinc-800 dark:bg-white dark:text-black dark:hover:bg-zinc-200 transition-colors cursor-pointer shadow-md">
                {#if appState.isPlaying} <Pause size={18} /> {:else} <Play size={18} class="ml-1" /> {/if}
            </button>
            <button onclick={() => appState.cmdNext++} class="text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer"><SkipForward size={20} /></button>
        </div>

        <div class="flex flex-1 items-center justify-center px-4">
            {#if appState.currentSample}
                <div class="flex items-center gap-4 border border-zinc-200 dark:border-zinc-700/50 rounded-lg p-1.5 px-3 bg-zinc-50 dark:bg-zinc-800/50 shadow-sm min-w-[350px]">
                    <div class="h-8 w-8 rounded overflow-hidden bg-zinc-200 dark:bg-zinc-700 flex items-center justify-center text-zinc-500 dark:text-zinc-400 shrink-0">
                        {#if appState.currentSample.cover_path}
                            <img src={convertFileSrc(appState.currentSample.cover_path)} alt="" class="h-full w-full object-cover" />
                        {:else}
                            <ImageIcon size={16} />
                        {/if}
                    </div>
                    <div class="flex flex-col max-w-[200px] min-w-[150px]">
                        <span class="text-sm font-bold truncate dark:text-zinc-100">{appState.currentSample.filename}</span>
                        <span class="text-[10px] text-zinc-500 uppercase font-semibold tracking-wider">{appState.currentSample.instrument_type || 'Audio'}</span>
                    </div>

                    <div class="h-6 w-px bg-zinc-200 dark:bg-zinc-700"></div>

                    <div class="flex items-center gap-3 px-2 text-xs font-semibold text-zinc-500 dark:text-zinc-400">
                        <span class="w-8 text-center">{appState.currentSample.key_signature || '--'}</span>
                        <span class="w-12 text-center">{appState.currentSample.bpm ? Math.round(appState.currentSample.bpm) : '--'} BPM</span>
                    </div>

                    <div class="h-6 w-px bg-zinc-200 dark:bg-zinc-700"></div>

                    <button onclick={() => invoke('reveal_in_finder', { path: appState.currentSample.original_path })} class="pl-2 pr-1 text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer" title="Reveal in Finder">
                        <FolderOpen size={16} />
                    </button>
                </div>
            {:else}
                <div class="text-xs font-medium text-zinc-400 uppercase tracking-widest">No Sample Selected</div>
            {/if}
        </div>

        <div class="flex items-center justify-end gap-3 w-1/4 text-zinc-500 dark:text-zinc-400">
            <Volume2 size={18} />
            <input
                    type="range" min="0" max="1" step="0.01"
                    bind:value={appState.globalVolume}
                    class="w-24 h-1.5 rounded-full appearance-none bg-zinc-200 dark:bg-zinc-700 accent-zinc-900 dark:accent-zinc-100 cursor-pointer"
            >
        </div>
    </footer>
</div>

{#if appState.isCreateCollectionModalOpen}
    <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm">
        <div class="w-full max-w-sm overflow-hidden rounded-xl border border-zinc-200 bg-white shadow-2xl dark:border-zinc-800 dark:bg-[#18181b] p-5">
            <h2 class="text-lg font-bold text-zinc-900 dark:text-zinc-100 mb-4">New Collection</h2>
            <input
                    type="text"
                    bind:value={newCollectionName}
                    placeholder="Collection Name"
                    class="w-full rounded-md border border-zinc-300 bg-white px-3 py-2 text-sm focus:border-emerald-500 focus:outline-none dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100 mb-5"
                    autofocus
                    onkeydown={(e) => { if (e.key === 'Enter') submitCollection(); }}
            >
            <div class="flex justify-end gap-3">
                <button onclick={() => { appState.isCreateCollectionModalOpen = false; newCollectionName = ''; }} class="px-4 py-2 text-sm font-semibold text-zinc-600 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100 cursor-pointer">Cancel</button>
                <button onclick={submitCollection} class="rounded-md bg-zinc-900 px-4 py-2 text-sm font-semibold text-white hover:bg-zinc-800 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-white cursor-pointer">Create</button>
            </div>
        </div>
    </div>
{/if}