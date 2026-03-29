<script lang="ts">
    import '../app.css';
    import { onMount } from 'svelte';
    import {
        Compass, Folder, Heart, Plus, Search, Play, Pause,
        SkipBack, SkipForward, Volume2, Sun, Moon, LayoutGrid, Library
    } from 'lucide-svelte';

    let { children } = $props();
    let isDarkMode = $state(true);

    onMount(() => {
        // Initiale Theme-Prüfung anhand der Systemeinstellungen
        if (window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches) {
            isDarkMode = false;
        }
        applyTheme();
    });

    function toggleTheme() {
        isDarkMode = !isDarkMode;
        applyTheme();
    }

    function applyTheme() {
        if (isDarkMode) {
            document.documentElement.classList.add('dark');
        } else {
            document.documentElement.classList.remove('dark');
        }
    }
</script>

<div class="flex h-screen w-full flex-col overflow-hidden bg-white text-zinc-900 dark:bg-[#121212] dark:text-zinc-100 font-sans antialiased transition-colors duration-200">

    <div class="flex flex-1 overflow-hidden pb-16">
        <aside class="flex w-64 flex-col border-r border-zinc-200 bg-zinc-50 dark:border-zinc-800/60 dark:bg-[#18181b]">
            <div class="flex h-14 items-center px-6 border-b border-zinc-200 dark:border-zinc-800/60">
                <span class="font-semibold tracking-tight text-lg">SampleVault</span>
            </div>

            <nav class="flex-1 overflow-y-auto px-3 py-4 space-y-6">
                <div class="space-y-1">
                    <button class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium bg-zinc-200/50 dark:bg-zinc-800/50 text-zinc-900 dark:text-zinc-50">
                        <Library size={18} />
                        Sounds
                    </button>
                    <button class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100 transition-colors">
                        <Compass size={18} />
                        Discover
                    </button>
                </div>

                <div>
                    <div class="mb-2 flex items-center justify-between px-3 text-xs font-semibold uppercase tracking-wider text-zinc-500">
                        <span>Collections</span>
                        <button class="hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors">
                            <Plus size={14} />
                        </button>
                    </div>
                    <div class="space-y-1">
                        <button class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100 transition-colors">
                            <Heart size={18} />
                            Likes
                        </button>
                        <button class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100 transition-colors">
                            <Folder size={18} />
                            Cinematic FX
                        </button>
                        <button class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/30 dark:hover:text-zinc-100 transition-colors">
                            <Folder size={18} />
                            Drums
                        </button>
                    </div>
                </div>
            </nav>
        </aside>

        <div class="flex flex-1 flex-col relative min-w-0">
            <header class="sticky top-0 z-10 flex h-14 items-center justify-between border-b border-zinc-200 bg-white/80 px-6 backdrop-blur-md dark:border-zinc-800/60 dark:bg-[#121212]/80">
                <div class="flex max-w-md flex-1 items-center relative">
                    <Search size={16} class="absolute left-3 text-zinc-400" />
                    <input
                        type="text"
                        placeholder="Search your library..."
                        class="h-9 w-full rounded-md border border-zinc-200 bg-zinc-50 pl-9 pr-4 text-sm outline-none focus:border-zinc-300 dark:border-zinc-800 dark:bg-zinc-900 dark:focus:border-zinc-700 transition-colors"
                    >
                </div>

                <div class="flex items-center gap-4 ml-4">
                    <button onclick={toggleTheme} class="p-2 text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100 transition-colors">
                        {#if isDarkMode}
                            <Sun size={18} />
                        {:else}
                            <Moon size={18} />
                        {/if}
                    </button>
                </div>
            </header>

            <main class="flex-1 overflow-y-auto">
                {@render children()}
            </main>
        </div>
    </div>

    <footer class="absolute bottom-0 left-0 z-50 flex h-16 w-full items-center justify-between border-t border-zinc-200 bg-white/90 px-6 backdrop-blur-xl dark:border-zinc-800/60 dark:bg-[#18181b]/90">
        <div class="flex items-center gap-4 w-1/3">
            <button class="text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors">
                <SkipBack size={20} />
            </button>
            <button class="flex h-10 w-10 items-center justify-center rounded-full bg-zinc-900 text-white hover:bg-zinc-800 dark:bg-white dark:text-black dark:hover:bg-zinc-200 transition-colors">
                <Play size={20} class="ml-1" />
            </button>
            <button class="text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors">
                <SkipForward size={20} />
            </button>
        </div>

        <div class="flex flex-1 items-center justify-center px-4">
            <div class="h-6 w-full max-w-2xl rounded bg-zinc-100 dark:bg-zinc-800/50"></div>
        </div>

        <div class="flex items-center justify-end gap-3 w-1/3 text-zinc-500 dark:text-zinc-400">
            <Volume2 size={18} />
            <div class="h-1.5 w-24 rounded-full bg-zinc-200 dark:bg-zinc-700">
                <div class="h-full w-2/3 rounded-full bg-zinc-900 dark:bg-zinc-100"></div>
            </div>
        </div>
    </footer>
</div>