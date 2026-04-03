<script lang="ts">
    import { appState } from '$lib/store.svelte';
    import { convertFileSrc, invoke } from '@tauri-apps/api/core';
    import { Play, Pause, SkipBack, SkipForward, Volume2, Image as ImageIcon, FolderOpen, FastForward, Rewind } from 'lucide-svelte';
    import { onMount, onDestroy } from 'svelte';

    let isVinylDropdownOpen = $state(false);

    function setVinylMode(speed: number) {
        appState.vinylSpeedMode = speed;
        isVinylDropdownOpen = false;
        if (typeof window !== 'undefined') {
            window.dispatchEvent(new CustomEvent('force-retrigger'));
        }
    }

    const handleVinylOutsideClick = (e: MouseEvent) => {
        const target = e.target as HTMLElement;
        if (!target.closest('.vinyl-dropdown-container')) {
            isVinylDropdownOpen = false;
        }
    };

    onMount(() => {
        window.addEventListener('click', handleVinylOutsideClick);
    });

    onDestroy(() => {
        if (typeof window !== 'undefined') {
            window.removeEventListener('click', handleVinylOutsideClick);
        }
    });
</script>

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
                    <span class="w-14 text-center whitespace-nowrap">{appState.currentSample.key_signature || '--'}</span>
                    <span class="w-16 text-center whitespace-nowrap">{appState.currentSample.bpm ? Math.round(appState.currentSample.bpm) : '--'} BPM</span>
                </div>

                <div class="h-6 w-px bg-zinc-200 dark:bg-zinc-700"></div>

                <button onclick={() => invoke('reveal_in_finder', { path: appState.currentSample!.original_path })} class="pl-2 pr-1 text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer" title="Reveal in Finder">
                    <FolderOpen size={16} />
                </button>
            </div>
        {:else}
            <div class="text-xs font-medium text-zinc-400 uppercase tracking-widest">No Sample Selected</div>
        {/if}
    </div>

    <div class="flex items-center justify-end gap-4 w-[30%] text-zinc-500 dark:text-zinc-400 pr-2">
        <div class="relative shrink-0 vinyl-dropdown-container">
            <button
                    onclick={(e) => { e.stopPropagation(); isVinylDropdownOpen = !isVinylDropdownOpen; }}
                    class="flex h-7 items-center gap-1.5 rounded-md border px-2.5 text-[10px] font-bold uppercase tracking-wider transition-all cursor-pointer shadow-sm
                {appState.vinylSpeedMode > 1.0 ? 'border-orange-500 bg-orange-500 text-white shadow-orange-500/30' : appState.vinylSpeedMode < 1.0 ? 'border-purple-500 bg-purple-500 text-white shadow-purple-500/30' : 'border-zinc-200 bg-zinc-100 text-zinc-500 hover:bg-zinc-200 dark:border-zinc-700/50 dark:bg-zinc-800 dark:text-zinc-400 dark:hover:bg-zinc-700'}"
            >
                {#if appState.vinylSpeedMode === 2.0}
                    <FastForward size={12} class="shrink-0" /><span>Kanye 2x</span>
                {:else if appState.vinylSpeedMode === 1.5}
                    <FastForward size={12} class="shrink-0" /><span>Fast 1.5x</span>
                {:else if appState.vinylSpeedMode === 0.5}
                    <Rewind size={12} class="shrink-0" /><span>Screwed 0.5x</span>
                {:else}
                    <span class="flex h-2.5 w-2.5 items-center justify-center rounded-full border-[1.5px] border-zinc-400"></span><span>Vinyl 1x</span>
                {/if}
            </button>

            {#if isVinylDropdownOpen}
                <div class="absolute bottom-full right-0 mb-2 w-40 flex-col rounded-xl border border-zinc-200 bg-white p-1.5 shadow-2xl dark:border-zinc-700/60 dark:bg-[#18181b] z-50 flex animate-in fade-in slide-in-from-bottom-2 duration-200">
                    <button onclick={() => setVinylMode(2.0)} class="flex items-center justify-between rounded-md px-3 py-2 text-xs font-bold {appState.vinylSpeedMode === 2.0 ? 'bg-orange-50 text-orange-600 dark:bg-orange-900/20 dark:text-orange-400' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800'} transition-colors cursor-pointer">
                        <span class="flex items-center gap-2"><FastForward size={14} /> Kanye</span><span class="text-[10px] opacity-70">2.0x</span>
                    </button>
                    <button onclick={() => setVinylMode(1.5)} class="flex items-center justify-between rounded-md px-3 py-2 text-xs font-bold {appState.vinylSpeedMode === 1.5 ? 'bg-orange-50 text-orange-600 dark:bg-orange-900/20 dark:text-orange-400' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800'} transition-colors cursor-pointer">
                        <span class="flex items-center gap-2"><FastForward size={14} /> Fast</span><span class="text-[10px] opacity-70">1.5x</span>
                    </button>
                    <div class="my-1 border-t border-zinc-100 dark:border-zinc-800/50"></div>
                    <button onclick={() => setVinylMode(1.0)} class="flex items-center justify-between rounded-md px-3 py-2 text-xs font-bold {appState.vinylSpeedMode === 1.0 ? 'bg-zinc-100 text-zinc-900 dark:bg-zinc-800 dark:text-white' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800'} transition-colors cursor-pointer">
                        <span class="flex items-center gap-2"><span class="flex h-3 w-3 items-center justify-center rounded-full border-[1.5px] border-current"></span> Normal</span><span class="text-[10px] opacity-70">1.0x</span>
                    </button>
                    <div class="my-1 border-t border-zinc-100 dark:border-zinc-800/50"></div>
                    <button onclick={() => setVinylMode(0.5)} class="flex items-center justify-between rounded-md px-3 py-2 text-xs font-bold {appState.vinylSpeedMode === 0.5 ? 'bg-purple-50 text-purple-600 dark:bg-purple-900/20 dark:text-purple-400' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800'} transition-colors cursor-pointer">
                        <span class="flex items-center gap-2"><Rewind size={14} /> Screwed</span><span class="text-[10px] opacity-70">0.5x</span>
                    </button>
                </div>
            {/if}
        </div>

        <div class="h-4 w-px bg-zinc-200 dark:bg-zinc-700"></div>

        <Volume2 size={18} />
        <input
                type="range" min="0" max="1" step="0.01"
                bind:value={appState.globalVolume}
                class="w-24 h-1.5 rounded-full appearance-none bg-zinc-200 dark:bg-zinc-700 accent-zinc-900 dark:accent-zinc-100 cursor-pointer"
        >
    </div>
</footer>