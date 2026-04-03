<script lang="ts">
    import '../app.css';
    import { onMount } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { appState } from '$lib/store.svelte';
    import { dragRegion } from '$lib/utils/window';
    import Header from '$lib/components/layout/Header.svelte';
    import PlayerFooter from '$lib/components/layout/PlayerFooter.svelte';
    import { Library, Plus, Heart, Folder } from 'lucide-svelte';

    let { children } = $props();

    let systemMedia: MediaQueryList;
    let isResizingSidebar = $state(false);

    function startSidebarResize() {
        isResizingSidebar = true;
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
        window.addEventListener('mousemove', handleSidebarResize);
        window.addEventListener('mouseup', stopSidebarResize);
    }

    function handleSidebarResize(e: MouseEvent) {
        if (!isResizingSidebar) return;
        let newWidth = e.clientX;
        if (newWidth < 200) newWidth = 200;
        if (newWidth > 600) newWidth = 600;
        appState.sidebarWidth = newWidth;
    }

    function stopSidebarResize() {
        isResizingSidebar = false;
        document.body.style.cursor = '';
        document.body.style.userSelect = '';
        window.removeEventListener('mousemove', handleSidebarResize);
        window.removeEventListener('mouseup', stopSidebarResize);
        localStorage.setItem('samplevault-sidebar-width', appState.sidebarWidth.toString());
    }

    $effect(() => {
        if (typeof window !== 'undefined' && appState.autoPlayEnabled !== undefined) {
            localStorage.setItem('samplevault-autoplay', appState.autoPlayEnabled.toString());
        }
    });

    onMount(async() => {
        const savedTheme = localStorage.getItem('samplevault-theme') as 'light' | 'dark' | 'system' | null;
        if (savedTheme) appState.themePreference = savedTheme;

        const savedVolume = localStorage.getItem('samplevault-volume');
        if (savedVolume !== null) {
            const parsedVolume = parseFloat(savedVolume);
            if (!isNaN(parsedVolume) && parsedVolume >= 0 && parsedVolume <= 1) {
                appState.globalVolume = parsedVolume;
            }
        }

        const savedWidth = localStorage.getItem('samplevault-sidebar-width');
        if (savedWidth) {
            const w = parseInt(savedWidth, 10);
            if (!isNaN(w)) appState.sidebarWidth = w;
        }

        const savedAutoPlay = localStorage.getItem('samplevault-autoplay');
        if (savedAutoPlay !== null) {
            appState.autoPlayEnabled = savedAutoPlay === 'true';
        }

        systemMedia = window.matchMedia('(prefers-color-scheme: dark)');
        systemMedia.addEventListener('change', applyTheme);

        applyTheme();
        await loadCollections();
    });

    async function loadCollections() {
        try {
            appState.collections = await invoke('get_collections');
        } catch (e) { console.error(e); }
    }

    $effect(() => {
        if (appState.themePreference) applyTheme();
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

    function switchToMainLibrary() {
        if (appState.currentView === 'sounds' && !appState.filters.onlyLiked && appState.filters.collectionId === null) return;
        appState.currentView = 'sounds';
        appState.activeSoundsTab = 'samples';
        appState.filters.onlyLiked = false;
        appState.filters.collectionId = null;
        if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent('trigger-sample-reload'));
    }

    function switchToLiked(enabled: boolean) {
        if (appState.filters.onlyLiked === enabled && appState.currentView === 'sounds' && appState.filters.collectionId === null) return;
        appState.currentView = 'sounds';
        appState.activeSoundsTab = 'collections';
        appState.filters.onlyLiked = enabled;
        appState.filters.collectionId = null;
        if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent('trigger-sample-reload'));
    }

    function switchToCollection(id: number | null) {
        if (appState.filters.collectionId === id && appState.currentView === 'sounds') return;
        appState.currentView = 'sounds';
        appState.activeSoundsTab = 'collections';
        appState.filters.collectionId = id;
        appState.filters.onlyLiked = false;
        if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent('trigger-sample-reload'));
    }
</script>

<div class="absolute inset-0 flex flex-col overflow-hidden bg-white text-zinc-900 dark:bg-[#121212] dark:text-zinc-100 font-sans antialiased" style="will-change: transform, width, height;">

    <div class="flex flex-1 overflow-hidden pb-16">
        <aside class="relative flex flex-col border-r border-zinc-200 bg-zinc-50 dark:border-zinc-800/60 dark:bg-[#18181b] shrink-0" style="width: {appState.sidebarWidth}px">
            <div use:dragRegion class="flex h-14 items-center pl-25 pr-6 border-b border-zinc-200 dark:border-zinc-800/60 shrink-0 select-none">
                <span class="font-bold tracking-tight text-lg">SampleVault</span>
            </div>

            <nav class="flex-1 overflow-y-auto px-3 py-4 space-y-6">
                {#if appState.currentView === 'sounds'}
                    <div class="space-y-1">
                        <button onclick={switchToMainLibrary} class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors cursor-pointer {!appState.filters.onlyLiked && appState.filters.collectionId === null ? 'bg-zinc-200/50 dark:bg-zinc-800/50 text-zinc-900 dark:text-zinc-50' : 'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100'}">
                            <Library size={18} /> Sounds
                        </button>
                    </div>

                    <div>
                        <div class="mb-2 flex items-center justify-between px-3 text-xs font-semibold uppercase tracking-wider text-zinc-500">
                            <span>Collections</span>
                            <button onclick={() => appState.isCreateCollectionModalOpen = true} class="hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer"><Plus size={14} /></button>
                        </div>
                        <div class="space-y-1">
                            <button onclick={() => { appState.filters.collectionId = null; switchToLiked(true); }} class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors cursor-pointer {appState.filters.onlyLiked ? 'bg-zinc-200/50 dark:bg-zinc-800/50 text-zinc-900 dark:text-zinc-50' : 'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100'}">
                                <Heart size={18} class={appState.filters.onlyLiked ? 'fill-red-500 text-red-500' : ''} /> Likes
                            </button>
                            {#each appState.collections as collection}
                                <button onclick={() => switchToCollection(collection.id)} class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors cursor-pointer {appState.filters.collectionId === collection.id ? 'bg-zinc-200/50 dark:bg-zinc-800/50 text-zinc-900 dark:text-zinc-50' : 'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100'}">
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
                        <button onclick={() => appState.activeSettingsTab = 'general'} class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors cursor-pointer {appState.activeSettingsTab === 'general' ? 'bg-zinc-200/50 text-zinc-900 dark:bg-zinc-800/50 dark:text-zinc-50' : 'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100'}">General</button>
                        <button onclick={() => appState.activeSettingsTab = 'library'} class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors cursor-pointer {appState.activeSettingsTab === 'library' ? 'bg-zinc-200/50 text-zinc-900 dark:bg-zinc-800/50 dark:text-zinc-50' : 'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100'}">Library</button>
                        <button onclick={() => appState.activeSettingsTab = 'audio'} class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors cursor-pointer {appState.activeSettingsTab === 'audio' ? 'bg-zinc-200/50 text-zinc-900 dark:bg-zinc-800/50 dark:text-zinc-50' : 'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100'}">Audio</button>
                        <button onclick={() => appState.activeSettingsTab = 'tags'} class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors cursor-pointer {appState.activeSettingsTab === 'tags' ? 'bg-zinc-200/50 text-zinc-900 dark:bg-zinc-800/50 dark:text-zinc-50' : 'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100'}">Tags</button>
                    </div>
                {/if}
            </nav>

            <div class="absolute top-0 bottom-0 -right-[5px] w-[10px] cursor-col-resize z-50 flex justify-center group" onmousedown={startSidebarResize}>
                <div class="w-px h-full bg-emerald-500 opacity-0 group-hover:opacity-100 transition-opacity"></div>
            </div>
        </aside>

        <div class="flex flex-1 flex-col relative min-w-0">
            <Header />
            <main class="flex-1 overflow-y-auto">{@render children()}</main>
        </div>
    </div>

    <PlayerFooter />
</div>