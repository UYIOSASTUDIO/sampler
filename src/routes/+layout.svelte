<script lang="ts">
    import '../app.css';
    import { onMount } from 'svelte';
    import { appState } from '$lib/store.svelte';
    import { getCurrentWindow } from '@tauri-apps/api/window'; // NEU: Direkte Tauri Window API
    import { invoke } from '@tauri-apps/api/core';
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

    onMount(() => {
        const savedTheme = localStorage.getItem('samplevault-theme') as 'light' | 'dark' | 'system' | null;
        if (savedTheme) {
            appState.themePreference = savedTheme;
        }

        systemMedia = window.matchMedia('(prefers-color-scheme: dark)');
        systemMedia.addEventListener('change', applyTheme);

        applyTheme();
    });

    $effect(() => {
        if (appState.themePreference) {
            applyTheme();
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
</script>

<div class="absolute inset-0 flex flex-col overflow-hidden bg-white text-zinc-900 dark:bg-[#121212] dark:text-zinc-100 font-sans antialiased" style="will-change: transform, width, height;">

    <div class="flex flex-1 overflow-hidden pb-16">

        <aside class="flex w-64 flex-col border-r border-zinc-200 bg-zinc-50 dark:border-zinc-800/60 dark:bg-[#18181b]">

            <div use:dragRegion class="flex h-14 items-center pl-25 pr-6 border-b border-zinc-200 dark:border-zinc-800/60 shrink-0 select-none">
                <span class="font-bold tracking-tight text-lg">SampleVault</span>
            </div>

            <nav class="flex-1 overflow-y-auto px-3 py-4 space-y-6">
                {#if appState.currentView === 'sounds'}
                    <div class="space-y-1">
                        <button class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium bg-zinc-200/50 dark:bg-zinc-800/50 text-zinc-900 dark:text-zinc-50 transition-colors cursor-pointer">
                            <Library size={18} /> Sounds
                        </button>
                        <button class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100 transition-colors cursor-pointer">
                            <Compass size={18} /> Discover
                        </button>
                    </div>

                    <div>
                        <div class="mb-2 flex items-center justify-between px-3 text-xs font-semibold uppercase tracking-wider text-zinc-500">
                            <span>Collections</span>
                            <button class="hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer">
                                <Plus size={14} />
                            </button>
                        </div>
                        <div class="space-y-1">
                            <button class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100 transition-colors cursor-pointer">
                                <Heart size={18} /> Likes
                            </button>
                            <button class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100 transition-colors cursor-pointer">
                                <Folder size={18} /> Cinematic FX
                            </button>
                            <button class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100 transition-colors cursor-pointer">
                                <Folder size={18} /> Drums
                            </button>
                        </div>
                    </div>

                {:else if appState.currentView === 'projects'}
                    <div class="mb-2 px-3 text-xs font-semibold uppercase tracking-wider text-zinc-500">Workspace</div>
                    <div class="px-3 text-sm text-zinc-400 italic">Project tools will appear here...</div>

                {:else if appState.currentView === 'editor'}
                    <div class="mb-2 px-3 text-xs font-semibold uppercase tracking-wider text-zinc-500">Batch Renamer</div>
                    <div class="px-3 text-sm text-zinc-400 italic">Metadata inputs will appear here...</div>
                {/if}
            </nav>
        </aside>

        <div class="flex flex-1 flex-col relative min-w-0">

            <header use:dragRegion class="sticky top-0 z-10 flex h-14 items-center justify-between border-b border-zinc-200 bg-white/80 px-6 backdrop-blur-md dark:border-zinc-800/60 dark:bg-[#121212]/80 shrink-0 select-none">

                <div class="flex flex-1 items-center relative">
                    <Search size={16} class="absolute left-3 text-zinc-400" />
                    <input data-tauri-drag-region="false" type="text" placeholder={appState.currentView === 'sounds' ? "Search your library..." : "Search..."} class="h-9 w-full max-w-sm rounded-md border border-zinc-200 bg-zinc-50 pl-9 pr-4 text-sm outline-none focus:border-zinc-300 dark:border-zinc-800 dark:bg-zinc-900 dark:focus:border-zinc-700 transition-colors">
                </div>

                <div class="flex items-center gap-6">
                    <div data-tauri-drag-region="false" class="flex rounded-lg bg-zinc-100 p-1 dark:bg-zinc-800/50 border border-zinc-200 dark:border-zinc-700/50">
                        <button onclick={() => appState.currentView = 'sounds'} class="flex items-center gap-2 rounded-md px-4 py-1.5 text-xs font-semibold transition-all cursor-pointer {appState.currentView === 'sounds' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}"><Library size={14} /> Sounds</button>
                        <button onclick={() => appState.currentView = 'projects'} class="flex items-center gap-2 rounded-md px-4 py-1.5 text-xs font-semibold transition-all cursor-pointer {appState.currentView === 'projects' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}"><Music size={14} /> Projects</button>
                        <button onclick={() => appState.currentView = 'editor'} class="flex items-center gap-2 rounded-md px-4 py-1.5 text-xs font-semibold transition-all cursor-pointer {appState.currentView === 'editor' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}"><Type size={14} /> Pack Editor</button>
                    </div>

                    <div class="h-5 w-px bg-zinc-200 dark:bg-zinc-700 pointer-events-none"></div>

                    <div data-tauri-drag-region="false" class="flex items-center gap-4">
                        <button onclick={toggleTheme} class="text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100 transition-colors cursor-pointer" title="Toggle Theme">
                            {#if appState.isDarkMode} <Sun size={18} /> {:else} <Moon size={18} /> {/if}
                        </button>
                        <button onclick={() => appState.isSettingsOpen = true} class="text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer" title="Preferences">
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
                    <div class="h-8 w-8 rounded bg-zinc-200 dark:bg-zinc-700 flex items-center justify-center text-zinc-500 dark:text-zinc-400">
                        <ImageIcon size={16} />
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