<script lang="ts">
    import { ChevronLeft, ChevronRight } from 'lucide-svelte';

    let {
        currentPage = $bindable(),
        totalPages,
        loadSamples
    }: {
        currentPage: number,
        totalPages: number,
        loadSamples: () => void
    } = $props();

    let visiblePages = $derived.by(() => {
        let pages = [];
        if (totalPages <= 5) {
            for (let i = 1; i <= totalPages; i++) pages.push(i);
        } else {
            if (currentPage <= 3) {
                pages = [1, 2, 3, 4, 5];
            } else if (currentPage >= totalPages - 2) {
                pages = [totalPages - 4, totalPages - 3, totalPages - 2, totalPages - 1, totalPages];
            } else {
                pages = [currentPage - 2, currentPage - 1, currentPage, currentPage + 1, currentPage + 2];
            }
        }
        return pages;
    });

    function goToPage(p: number) { if (p !== currentPage) { currentPage = p; loadSamples(); } }
    function nextPage() { if (currentPage < totalPages) { currentPage++; loadSamples(); } }
    function prevPage() { if (currentPage > 1) { currentPage--; loadSamples(); } }
</script>

{#if totalPages > 1}
    <div class="w-full">
        <div class="flex items-center justify-center pb-8 pt-4">
            <div class="flex items-center gap-1">
                <button onclick={prevPage} disabled={currentPage === 1} class="flex items-center justify-center h-8 w-8 rounded text-zinc-600 hover:bg-zinc-100 disabled:opacity-30 disabled:hover:bg-transparent dark:text-zinc-400 dark:hover:bg-zinc-800 transition-colors cursor-pointer mr-2"><ChevronLeft size={18} /></button>
                {#each visiblePages as pageNum}
                    <button onclick={() => goToPage(pageNum)} class="flex items-center justify-center h-8 w-8 rounded text-sm font-medium transition-colors cursor-pointer {pageNum === currentPage ? 'bg-zinc-900 text-white dark:bg-zinc-100 dark:text-zinc-900' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800'}">{pageNum}</button>
                {/each}
                <button onclick={nextPage} disabled={currentPage === totalPages} class="flex items-center justify-center h-8 w-8 rounded text-zinc-600 hover:bg-zinc-100 disabled:opacity-30 disabled:hover:bg-transparent dark:text-zinc-400 dark:hover:bg-zinc-800 transition-colors cursor-pointer ml-2"><ChevronRight size={18} /></button>
            </div>
        </div>
    </div>
{/if}