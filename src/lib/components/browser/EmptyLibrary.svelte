<script lang="ts">
    import { Search, Library, FolderPlus, RefreshCw, Play, Folder, X } from 'lucide-svelte';
    import { scannerState, getIsScanning, handleSelectFolder, handleScan } from '$lib/stores/scanner.svelte';

    let {
        hasActiveFilters,
        clearAllFilters,
        loadSamples
    }: {
        hasActiveFilters: boolean,
        clearAllFilters: () => void,
        loadSamples: () => void
    } = $props();
</script>

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
                <button onclick={handleSelectFolder} disabled={getIsScanning()} class="flex items-center gap-2 rounded-lg bg-zinc-900 px-6 py-3 text-sm font-bold text-white shadow-sm hover:bg-zinc-800 dark:bg-white dark:text-zinc-900 dark:hover:bg-zinc-200 transition-colors disabled:opacity-50 cursor-pointer">
                    <FolderPlus size={18} /> Select Folders
                </button>

                {#if scannerState.queue.length > 0}
                    <button onclick={() => handleScan(loadSamples)} disabled={getIsScanning()} class="flex items-center gap-2 rounded-lg bg-emerald-500 px-6 py-3 text-sm font-bold text-white shadow-sm hover:bg-emerald-600 transition-colors disabled:opacity-50 cursor-pointer animate-in fade-in slide-in-from-left-4 duration-300">
                        {#if scannerState.isScanningNew}
                            <RefreshCw size={18} class="animate-spin" /> Scanning...
                        {:else}
                            <Play size={18} /> Scan {scannerState.queue.length} {scannerState.queue.length === 1 ? 'Folder' : 'Folders'}
                        {/if}
                    </button>
                {/if}
            </div>

            {#if scannerState.message && scannerState.isScanningNew}
                <p class="mt-4 text-xs font-semibold text-emerald-600 dark:text-emerald-400 animate-pulse">{scannerState.message}</p>
            {/if}

            {#if scannerState.queue.length > 0}
                <div class="mt-10 w-full max-w-lg border-t border-zinc-200 dark:border-zinc-800/60 pt-6 animate-in fade-in slide-in-from-bottom-4 duration-300">
                    <h3 class="text-[10px] font-bold uppercase tracking-wider text-zinc-400 mb-3 flex items-center justify-between px-1">
                        <span>Warteschlange ({scannerState.queue.length})</span>
                        <button onclick={() => scannerState.queue = []} class="hover:text-red-500 transition-colors cursor-pointer">Clear All</button>
                    </h3>
                    <div class="flex flex-col gap-2 max-h-48 overflow-y-auto no-scrollbar">
                        {#each scannerState.queue as path}
                            <div class="flex items-center justify-between bg-white dark:bg-[#18181b] border border-zinc-200 dark:border-zinc-800/80 rounded-lg px-3 py-2.5 shadow-sm">
                                <div class="flex items-center gap-3 overflow-hidden">
                                    <Folder size={14} class="text-zinc-400 shrink-0" />
                                    <span class="text-xs font-bold text-zinc-700 dark:text-zinc-300 truncate" title={path}>{path.split(/[/\\]/).pop()}</span>
                                </div>
                                <button onclick={() => scannerState.queue = scannerState.queue.filter(p => p !== path)} class="text-zinc-400 hover:text-red-500 shrink-0 ml-2 transition-colors cursor-pointer">
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