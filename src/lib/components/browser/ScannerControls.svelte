<script lang="ts">
    import { Play, FolderPlus, RefreshCw, X, ChevronDown, Folder } from 'lucide-svelte';
    import { onMount, onDestroy } from 'svelte';
    import { scannerState, getIsScanning, getScanPercentage, handleRescanAll, handleSelectFolder, handleScan } from '$lib/stores/scanner.svelte';

    let { loadSamples }: { loadSamples: () => void } = $props();

    let isQueueDropdownOpen = $state(false);

    const handleOutsideClick = (e: MouseEvent) => {
        if (isQueueDropdownOpen) {
            const target = e.target as HTMLElement;
            if (!target.closest('.queue-dropdown-container')) {
                isQueueDropdownOpen = false;
            }
        }
    };

    onMount(() => window.addEventListener('click', handleOutsideClick));
    onDestroy(() => window.removeEventListener('click', handleOutsideClick));
</script>

<div class="flex flex-col items-end gap-3 pb-2 queue-dropdown-container">
    {#if getIsScanning() && scannerState.total > 0}
        <div class="w-full flex flex-col gap-1 mt-1">
            <div class="flex justify-between text-[10px] font-medium text-zinc-500 uppercase tracking-wider">
                <span>Scanning: {scannerState.current} / {scannerState.total}</span><span>{getScanPercentage()}%</span>
            </div>
            <div class="h-1.5 w-full rounded-full bg-zinc-200 overflow-hidden dark:bg-zinc-800">
                <div class="h-full bg-blue-500 transition-all duration-300 ease-out" style="width: {getScanPercentage()}%"></div>
            </div>
            <div class="text-[10px] text-zinc-400 truncate text-right">{scannerState.currentFile}</div>
        </div>
    {:else if scannerState.message}
        <span class="text-xs font-medium text-zinc-500 animate-pulse">{scannerState.message}</span>
    {/if}

    <div class="flex items-center gap-2">
        <button onclick={() => handleRescanAll(loadSamples)} disabled={getIsScanning()} class="flex h-8 items-center gap-1.5 rounded-md border border-blue-700/50 bg-blue-50 px-3 text-xs font-medium text-blue-700 hover:bg-blue-100 dark:border-blue-500/30 dark:bg-blue-500/10 dark:text-blue-400 cursor-pointer disabled:opacity-50 transition-colors">
            <RefreshCw size={14} class={scannerState.isSyncing ? "animate-spin" : ""} /> Sync
        </button>

        <div class="flex items-center">
            <button onclick={handleSelectFolder} disabled={getIsScanning()} class="flex h-8 items-center gap-1.5 rounded-md border border-zinc-200 bg-white px-3 text-xs font-medium hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-800 dark:hover:bg-zinc-700 transition-colors disabled:opacity-50 cursor-pointer z-10 shadow-sm">
                <FolderPlus size={14} /> Add Folder
            </button>

            {#if scannerState.queue.length > 0}
                <div class="relative ml-2 flex h-8 items-center shadow-sm rounded-md animate-in fade-in zoom-in-95 duration-200">
                    <button onclick={() => handleScan(loadSamples)} disabled={getIsScanning()} class="flex h-full items-center gap-1.5 rounded-l-md border border-emerald-700/50 bg-emerald-50 px-3 text-xs font-bold text-emerald-700 hover:bg-emerald-100 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-400 cursor-pointer disabled:opacity-50 transition-colors">
                        {#if scannerState.isScanningNew}
                            <RefreshCw size={14} class="animate-spin" /> Scanning...
                        {:else}
                            <Play size={14} /> Scan {scannerState.queue.length} {scannerState.queue.length === 1 ? 'Folder' : 'Folders'}
                        {/if}
                    </button>

                    <button onclick={(e) => { e.stopPropagation(); isQueueDropdownOpen = !isQueueDropdownOpen; }} disabled={getIsScanning()} class="flex h-full items-center justify-center border-y border-r border-emerald-700/50 bg-emerald-50 px-1.5 text-emerald-700 hover:bg-emerald-100 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-400 rounded-r-md cursor-pointer transition-colors disabled:opacity-50">
                        <ChevronDown size={14} />
                    </button>

                    {#if isQueueDropdownOpen}
                        <div onclick={(e) => e.stopPropagation()} class="absolute right-0 top-full mt-2 w-72 flex-col rounded-xl border border-zinc-200 bg-white p-1.5 shadow-2xl dark:border-zinc-700/60 dark:bg-[#18181b] z-50 flex">
                            <div class="flex items-center justify-between px-2 pb-2 pt-1 border-b border-zinc-100 dark:border-zinc-800">
                                <span class="text-[10px] font-bold uppercase tracking-wider text-zinc-500">Scan Queue</span>
                                <button onclick={() => { scannerState.queue = []; isQueueDropdownOpen = false; }} class="text-[10px] text-red-500 hover:text-red-600 uppercase font-bold tracking-wider cursor-pointer">Clear All</button>
                            </div>
                            <div class="max-h-48 overflow-y-auto no-scrollbar flex flex-col gap-0.5 mt-1.5">
                                {#each scannerState.queue as path}
                                    <div class="group flex items-center justify-between rounded-md px-2 py-1.5 text-xs hover:bg-zinc-50 dark:hover:bg-zinc-800 transition-colors">
                                        <div class="flex items-center gap-2 overflow-hidden">
                                            <Folder size={12} class="text-zinc-400 shrink-0" />
                                            <span class="truncate font-medium text-zinc-700 dark:text-zinc-300" title={path}>{path.split(/[/\\]/).pop()}</span>
                                        </div>
                                        <button onclick={() => { scannerState.queue = scannerState.queue.filter(p => p !== path); if(scannerState.queue.length === 0) isQueueDropdownOpen = false; }} class="opacity-0 group-hover:opacity-100 text-zinc-400 hover:text-red-500 transition-all cursor-pointer shrink-0 ml-2">
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