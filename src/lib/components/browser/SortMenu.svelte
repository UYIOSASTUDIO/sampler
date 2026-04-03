<script lang="ts">
    import { ChevronDown, ArrowDownUp, Shuffle } from 'lucide-svelte';
    import { onMount, onDestroy } from 'svelte';

    let {
        sortField = $bindable(),
        sortOrder = $bindable(),
        loadSamples
    }: {
        sortField: 'name' | 'type' | 'pack' | 'random',
        sortOrder: 'asc' | 'desc',
        loadSamples: () => void
    } = $props();

    let isSortDropdownOpen = $state(false);

    const handleOutsideClick = (e: MouseEvent) => {
        if (isSortDropdownOpen) {
            const target = e.target as HTMLElement;
            if (!target.closest('.sort-dropdown-container')) {
                isSortDropdownOpen = false;
            }
        }
    };

    onMount(() => window.addEventListener('click', handleOutsideClick));
    onDestroy(() => window.removeEventListener('click', handleOutsideClick));
</script>

<div class="flex items-center rounded-md border border-zinc-200 bg-white p-0.5 shadow-sm dark:border-zinc-700 dark:bg-zinc-900">
    <div class="relative sort-dropdown-container">
        <button onclick={(e) => { e.stopPropagation(); isSortDropdownOpen = !isSortDropdownOpen; }} class="flex h-7 items-center gap-2 rounded px-2 text-xs font-semibold text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800 transition-colors cursor-pointer">
            {sortField === 'name' ? 'Alphabetical' : sortField === 'type' ? 'Instrument Type' : sortField === 'pack' ? 'Sample Pack' : 'Randomize'}
            <ChevronDown size={14} class="opacity-50" />
        </button>
        {#if isSortDropdownOpen}
            <div onclick={(e) => e.stopPropagation()} class="absolute right-0 top-full mt-1 w-40 flex-col rounded-lg border border-zinc-200 bg-white p-1 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                <button onclick={() => { sortField = 'name'; isSortDropdownOpen = false; loadSamples(); }} class="w-full text-left rounded-md px-2 py-1.5 text-xs hover:bg-zinc-100 dark:hover:bg-zinc-800 cursor-pointer {sortField === 'name' ? 'font-bold text-zinc-900 dark:text-white' : ''}">Alphabetical</button>
                <button onclick={() => { sortField = 'type'; isSortDropdownOpen = false; loadSamples(); }} class="w-full text-left rounded-md px-2 py-1.5 text-xs hover:bg-zinc-100 dark:hover:bg-zinc-800 cursor-pointer {sortField === 'type' ? 'font-bold text-zinc-900 dark:text-white' : ''}">Instrument Type</button>
                <button onclick={() => { sortField = 'pack'; isSortDropdownOpen = false; loadSamples(); }} class="w-full text-left rounded-md px-2 py-1.5 text-xs hover:bg-zinc-100 dark:hover:bg-zinc-800 cursor-pointer {sortField === 'pack' ? 'font-bold text-zinc-900 dark:text-white' : ''}">Sample Pack</button>
            </div>
        {/if}
    </div>
    <div class="h-4 w-px bg-zinc-200 dark:bg-zinc-700 mx-0.5"></div>
    <button onclick={() => { sortOrder = sortOrder === 'asc' ? 'desc' : 'asc'; if(sortField !== 'random') { loadSamples(); } }} disabled={sortField === 'random'} class="flex h-7 w-7 items-center justify-center rounded text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 disabled:opacity-30 disabled:hover:bg-transparent dark:hover:bg-zinc-800 dark:hover:text-zinc-100 transition-colors cursor-pointer" title="Reverse Order"><ArrowDownUp size={14} class={sortOrder === 'desc' ? 'rotate-180 transition-transform' : 'transition-transform'} /></button>
    <button onclick={() => { sortField = 'random'; loadSamples(); }} class="flex h-7 w-7 items-center justify-center rounded text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-zinc-100 transition-colors cursor-pointer {sortField === 'random' ? 'bg-zinc-200 text-zinc-900 dark:bg-zinc-700 dark:text-zinc-100 shadow-inner' : ''}" title="Shuffle"><Shuffle size={14} /></button>
</div>