<script lang="ts">
    import { Heart, Folder, Plus } from 'lucide-svelte';
    import { appState } from '$lib/store.svelte';

    let {
        currentPage = $bindable(),
        loadSamples
    }: {
        currentPage: number,
        loadSamples: () => void
    } = $props();
</script>

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