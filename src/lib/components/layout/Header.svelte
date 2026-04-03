<script lang="ts">
    import { appState } from '$lib/store.svelte';
    import { dragRegion } from '$lib/utils/window';
    import { Search, Library, Music, Type, Settings, Sun, Moon } from 'lucide-svelte';

    let windowWidth = $state(0);
    let screenWidth = $state(0);

    let showTabLabels = $derived(screenWidth > 0 ? windowWidth > (screenWidth * 0.75) : true);

    function toggleTheme() {
        appState.themePreference = appState.isDarkMode ? 'light' : 'dark';
        localStorage.setItem('samplevault-theme', appState.themePreference);
    }

    $effect(() => {
        if (typeof window !== 'undefined') {
            screenWidth = window.screen.availWidth;
            windowWidth = window.innerWidth;
        }
    });
</script>

<svelte:window bind:innerWidth={windowWidth} />

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