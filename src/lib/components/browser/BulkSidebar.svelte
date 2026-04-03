<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { Heart, Folder, Plus, RefreshCw, X, ChevronDown } from 'lucide-svelte';
    import { appState } from '$lib/store.svelte';
    import { parseTags } from '$lib/utils/tags';
    import type { SampleRecord } from '$lib/types';
    import { onMount, onDestroy } from 'svelte';

    let {
        samples = $bindable(),
        allAvailableTags,
        loadAllTags
    }: {
        samples: SampleRecord[],
        allAvailableTags: Array<{category: string, value: string}>,
        loadAllTags: () => Promise<void>
    } = $props();

    let isBulkTagDropdownOpen = $state(false);
    let bulkTagSearchQuery = $state('');
    let isBulkTagging = $state(false);

    let selectedSampleRecords = $derived(samples.filter(s => appState.selectedSampleIds.includes(s.id)));

    let commonTags = $derived.by(() => {
        if (selectedSampleRecords.length === 0) return [];
        let shared = parseTags(selectedSampleRecords[0].tags);
        for (let i = 1; i < selectedSampleRecords.length; i++) {
            const currentTags = parseTags(selectedSampleRecords[i].tags);
            shared = shared.filter(st => currentTags.some(ct => ct.category === st.category && ct.value === st.value));
        }
        return shared;
    });

    let filteredBulkTags = $derived(
        allAvailableTags.filter(t => t.value.toLowerCase().includes(bulkTagSearchQuery.toLowerCase()))
    );

    async function handleBulkAddToCollection(collectionId: number) {
        const ids = [...appState.selectedSampleIds];
        appState.selectedSampleIds = [];
        try { await invoke('add_to_collection', { collectionId, sampleIds: ids }); }
        catch (e) { console.error(e); }
    }

    async function handleBulkLike() {
        const ids = [...appState.selectedSampleIds];
        samples.forEach(s => { if (ids.includes(s.id)) s.is_liked = true; });
        samples = [...samples];
        appState.selectedSampleIds = [];
        try { await invoke('bulk_toggle_like', { sampleIds: ids, isLiked: true }); }
        catch (e) { console.error(e); }
    }

    async function handleBulkAddTag(category: string, value: string) {
        isBulkTagDropdownOpen = false;
        bulkTagSearchQuery = '';
        isBulkTagging = true;
        const tagToAdd = { category, value };
        const updates = [];

        for (let sample of selectedSampleRecords) {
            let currentTags = parseTags(sample.tags);
            if (!currentTags.some(t => t.category === category && t.value === value)) {
                currentTags.push(tagToAdd);
                sample.tags = JSON.stringify(currentTags);
                updates.push(invoke('update_sample_metadata', {
                    payload: { id: sample.id, filename: sample.filename, bpm: sample.bpm, keySignature: sample.key_signature, tags: sample.tags }
                }));
            }
        }
        samples = [...samples];
        try { await Promise.all(updates); } catch(e) { console.error(e); } finally { isBulkTagging = false; }
    }

    async function handleBulkRemoveTag(category: string, value: string) {
        isBulkTagging = true;
        const updates = [];
        for (let sample of selectedSampleRecords) {
            let currentTags = parseTags(sample.tags);
            const originalLength = currentTags.length;
            currentTags = currentTags.filter(t => !(t.category === category && t.value === value));
            if (currentTags.length < originalLength) {
                sample.tags = JSON.stringify(currentTags);
                updates.push(invoke('update_sample_metadata', {
                    payload: { id: sample.id, filename: sample.filename, bpm: sample.bpm, keySignature: sample.key_signature, tags: sample.tags }
                }));
            }
        }
        samples = [...samples];
        try { await Promise.all(updates); } catch(e) { console.error(e); } finally { isBulkTagging = false; }
    }

    async function createNewBulkTag() {
        const trimmed = bulkTagSearchQuery.trim();
        if (trimmed === '') return;
        try {
            await invoke('create_user_tag', { category: 'User', value: trimmed });
            await loadAllTags();
            await handleBulkAddTag('User', trimmed);
        } catch (e) { console.error(e); }
    }

    // Component-spezifischer Listener, schließt Dropdown bei Klick daneben
    const handleOutsideClick = (e: MouseEvent) => {
        if (isBulkTagDropdownOpen) {
            const target = e.target as HTMLElement;
            if (!target.closest('.bulk-tag-dropdown-container')) {
                isBulkTagDropdownOpen = false;
            }
        }
    };

    onMount(() => window.addEventListener('click', handleOutsideClick));
    onDestroy(() => window.removeEventListener('click', handleOutsideClick));
</script>

<div class="absolute right-0 top-0 bottom-0 w-72 bg-white/95 backdrop-blur-xl border-l border-zinc-200 dark:border-zinc-800/60 dark:bg-[#18181b]/95 shadow-2xl transition-transform duration-300 flex flex-col z-50 {appState.selectedSampleIds.length > 0 ? 'translate-x-0' : 'translate-x-full'}">
    <div class="flex items-center justify-between px-5 py-4 border-b border-zinc-200 dark:border-zinc-800/60">
        <span class="font-bold text-sm">{appState.selectedSampleIds.length} Items Selected</span>
        <button onclick={() => appState.selectedSampleIds = []} class="text-zinc-400 hover:text-zinc-900 dark:hover:text-white cursor-pointer"><X size={16} /></button>
    </div>
    <div class="flex-1 overflow-y-auto p-3 space-y-1 no-scrollbar">

        <span class="px-2 text-[10px] font-bold uppercase tracking-wider text-zinc-400">Add to...</span>
        <div class="flex w-full items-center justify-between rounded-md px-2 py-1.5 transition-colors hover:bg-zinc-50 dark:hover:bg-zinc-800/30">
            <div class="flex items-center gap-3 text-sm font-medium text-zinc-600 dark:text-zinc-400"><Heart size={16} /> Liked Folder</div>
            <button onclick={handleBulkLike} class="rounded border border-zinc-200 bg-white px-3 py-1 text-xs font-semibold text-zinc-700 shadow-sm transition-colors hover:bg-zinc-100 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700 cursor-pointer">Add</button>
        </div>
        <div class="my-2 border-t border-zinc-200 dark:border-zinc-800/50"></div>
        {#each appState.collections as collection}
            <div class="flex w-full items-center justify-between rounded-md px-2 py-1.5 transition-colors hover:bg-zinc-50 dark:hover:bg-zinc-800/30">
                <div class="flex items-center gap-3 text-sm font-medium text-zinc-600 dark:text-zinc-400"><Folder size={16} /> {collection.name}</div>
                <button onclick={() => handleBulkAddToCollection(collection.id)} class="rounded border border-zinc-200 bg-white px-3 py-1 text-xs font-semibold text-zinc-700 shadow-sm transition-colors hover:bg-zinc-100 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700 cursor-pointer">Add</button>
            </div>
        {/each}
        <div class="my-2 border-t border-zinc-200 dark:border-zinc-800/50"></div>
        <button onclick={() => appState.isCreateCollectionModalOpen = true} class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/50 dark:hover:text-zinc-100 transition-colors cursor-pointer"><Plus size={16} /> New Collection</button>

        <div class="my-4 border-t border-zinc-200 dark:border-zinc-800/50"></div>
        <div class="flex items-center justify-between px-2">
            <span class="text-[10px] font-bold uppercase tracking-wider text-zinc-400">Shared Tags</span>
            {#if isBulkTagging}<RefreshCw size={12} class="animate-spin text-zinc-400" />{/if}
        </div>

        <div class="px-2 mt-2">
            <div class="flex flex-wrap gap-1.5 mb-4 min-h-[24px]">
                {#each commonTags as tag}
                    <div class="group relative flex items-center justify-center h-6 rounded-full border border-zinc-200 bg-zinc-50 px-2.5 text-[10px] font-semibold text-zinc-600 transition-all hover:pr-6 dark:border-zinc-800 dark:bg-[#18181b] dark:text-zinc-300 cursor-default overflow-hidden shadow-sm">
                        <span>{tag.value}</span>
                        <button onclick={() => handleBulkRemoveTag(tag.category, tag.value)} class="absolute right-1 opacity-0 group-hover:opacity-100 flex h-4 w-4 items-center justify-center rounded-full bg-zinc-200 text-zinc-600 hover:bg-red-500 hover:text-white dark:bg-zinc-700 dark:text-zinc-300 dark:hover:bg-red-600 transition-all cursor-pointer"><X size={10} /></button>
                    </div>
                {/each}
                {#if commonTags.length === 0}
                    <span class="text-[10px] text-zinc-500 italic flex items-center h-6">No tags shared by all selected.</span>
                {/if}
            </div>

            <div class="relative bulk-tag-dropdown-container">
                <button onclick={(e) => { e.stopPropagation(); isBulkTagDropdownOpen = !isBulkTagDropdownOpen; }} class="flex w-full items-center justify-between rounded-md border border-zinc-300 bg-white px-3 py-2 text-xs font-semibold text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-300 dark:hover:bg-zinc-800 transition-colors cursor-pointer shadow-sm">
                    <span class="flex items-center gap-2"><Plus size={14} /> Add Tag to All</span>
                    <ChevronDown size={14} class="opacity-50" />
                </button>

                {#if isBulkTagDropdownOpen}
                    <div onclick={(e) => e.stopPropagation()} class="absolute left-0 top-full mt-2 w-full flex-col rounded-lg border border-zinc-200 bg-white p-2 shadow-2xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                        <input type="text" bind:value={bulkTagSearchQuery} placeholder="Search tags..." class="w-full rounded-md border border-zinc-200 bg-zinc-50 px-2 py-1.5 text-xs focus:border-emerald-500 focus:outline-none dark:border-zinc-700 dark:bg-zinc-900 dark:text-white transition-colors mb-2" autofocus />

                        {#if bulkTagSearchQuery.trim() !== '' && !allAvailableTags.some(t => t.value.toLowerCase() === bulkTagSearchQuery.trim().toLowerCase())}
                            <button onclick={createNewBulkTag} class="w-full mb-2 flex items-center justify-center gap-1.5 rounded-md bg-emerald-50 text-emerald-600 px-2 py-1.5 text-xs font-bold hover:bg-emerald-100 dark:bg-emerald-900/20 dark:text-emerald-400 dark:hover:bg-emerald-900/40 transition-colors cursor-pointer shadow-sm">
                                <Plus size={12} /> Create global Tag "{bulkTagSearchQuery}"
                            </button>
                        {/if}

                        <div class="max-h-48 overflow-y-auto no-scrollbar flex flex-col gap-0.5 border-t border-zinc-100 dark:border-zinc-800/50 pt-2">
                            {#each filteredBulkTags as tag}
                                <button
                                        onclick={() => handleBulkAddTag(tag.category, tag.value)}
                                        class="w-full text-left flex items-center justify-between rounded-md px-2 py-1.5 text-xs font-medium text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800 cursor-pointer transition-colors"
                                >
                                    <span class="truncate pr-2">{tag.value}</span>
                                    <span class="text-[8px] uppercase text-zinc-400 font-bold tracking-wider shrink-0">{tag.category}</span>
                                </button>
                            {/each}
                            {#if filteredBulkTags.length === 0 && bulkTagSearchQuery.trim() === ''}
                                <span class="text-xs text-zinc-500 italic p-2 text-center">No tags found.</span>
                            {/if}
                        </div>
                    </div>
                {/if}
            </div>
        </div>
    </div>
</div>