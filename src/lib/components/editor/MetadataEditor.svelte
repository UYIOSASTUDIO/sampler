<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { Plus, X } from 'lucide-svelte';
    import type { SampleRecord } from '$lib/types';
    import { parseTags } from '$lib/utils/tags';

    let {
        editingSample = $bindable(),
        samples = $bindable(),
        allAvailableTags,
        loadAllTags
    }: {
        editingSample: SampleRecord | null,
        samples: SampleRecord[],
        allAvailableTags: Array<{category: string, value: string}>,
        loadAllTags: () => Promise<void>
    } = $props();

    // Wir initialisieren das editForm direkt aus dem übergebenen Sample
    let editForm = $state({
        filename: editingSample?.filename || '',
        bpm: editingSample?.bpm || null,
        key_signature: editingSample?.key_signature || '',
        tags: editingSample ? parseTags(editingSample.tags) : []
    });

    let isTagDropdownOpen: boolean = $state(false);
    let tagSearchQuery: string = $state('');

    let filteredTagsForEditor = $derived(
        allAvailableTags.filter(t => t.value.toLowerCase().includes(tagSearchQuery.toLowerCase()))
    );

    function removeTagFromEditor(index: number) {
        editForm.tags.splice(index, 1);
        editForm.tags = [...editForm.tags];
    }

    function addTagToEditor(category: string, value: string) {
        if (!editForm.tags.some(t => t.value === value && t.category === category)) {
            editForm.tags.push({ category, value });
            editForm.tags = [...editForm.tags];
        }
        isTagDropdownOpen = false;
        tagSearchQuery = '';
    }

    async function createNewTag() {
        const trimmed = tagSearchQuery.trim();
        if (trimmed === '') return;
        try {
            await invoke('create_user_tag', { category: 'User', value: trimmed });
            await loadAllTags();
            addTagToEditor('User', trimmed);
        } catch (e) { console.error(e); }
    }

    async function saveMetadata() {
        if (!editingSample) return;
        try {
            const tagsJson = JSON.stringify(editForm.tags);
            await invoke('update_sample_metadata', {
                payload: {
                    id: editingSample.id,
                    filename: editForm.filename.trim(),
                    bpm: editForm.bpm,
                    keySignature: editForm.key_signature.trim() !== '' ? editForm.key_signature.trim() : null,
                    tags: tagsJson
                }
            });

            // State im Parent aktualisieren
            const index = samples.findIndex(s => s.id === editingSample!.id);
            if (index !== -1) {
                samples[index].filename = editForm.filename.trim();
                samples[index].bpm = editForm.bpm;
                samples[index].key_signature = editForm.key_signature.trim() !== '' ? editForm.key_signature.trim() : null;
                samples[index].tags = tagsJson;
            }
            samples = [...samples];
            editingSample = null;
        } catch(e) { console.error("Failed to save metadata:", e); }
    }
</script>

<div class="fixed inset-0 z-[60] flex items-center justify-center bg-black/40 backdrop-blur-sm" onclick={() => editingSample = null}>
    <div class="w-full max-w-md rounded-xl border border-zinc-200 bg-white shadow-2xl dark:border-zinc-800 dark:bg-[#18181b] p-6" onclick={(e) => e.stopPropagation()} role="dialog" tabindex="-1">
        <h2 class="text-lg font-bold text-zinc-900 dark:text-zinc-100 mb-6">Edit Properties</h2>
        <div class="space-y-4">
            <div>
                <label class="block text-[10px] font-bold text-zinc-500 uppercase tracking-wider mb-1.5">Filename</label>
                <input type="text" bind:value={editForm.filename} class="w-full rounded-md border border-zinc-300 bg-white px-3 py-2 text-sm focus:border-zinc-900 focus:outline-none dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100 dark:focus:border-zinc-100 transition-colors" />
            </div>
            <div class="flex gap-4">
                <div class="flex-1">
                    <label class="block text-[10px] font-bold text-zinc-500 uppercase tracking-wider mb-1.5">BPM</label>
                    <input type="number" bind:value={editForm.bpm} placeholder="e.g. 120" class="w-full rounded-md border border-zinc-300 bg-white px-3 py-2 text-sm focus:border-zinc-900 focus:outline-none dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100 dark:focus:border-zinc-100 transition-colors" />
                </div>
                <div class="flex-1">
                    <label class="block text-[10px] font-bold text-zinc-500 uppercase tracking-wider mb-1.5">Key</label>
                    <input type="text" bind:value={editForm.key_signature} placeholder="e.g. F# min" class="w-full rounded-md border border-zinc-300 bg-white px-3 py-2 text-sm focus:border-zinc-900 focus:outline-none dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100 dark:focus:border-zinc-100 transition-colors" />
                </div>
            </div>
            <div>
                <label class="block text-[10px] font-bold text-zinc-500 uppercase tracking-wider mb-1.5">Tags</label>
                <div class="flex flex-wrap gap-2 items-center min-h-[42px] rounded-md border border-zinc-300 bg-white px-3 py-2 dark:border-zinc-700 dark:bg-zinc-900 transition-colors">
                    {#each editForm.tags as tag, i}
                        <div class="group relative flex items-center justify-center h-6 rounded-full border border-zinc-200 bg-zinc-50 px-2.5 text-[11px] font-semibold text-zinc-600 transition-all hover:pr-6 dark:border-zinc-800 dark:bg-[#18181b] dark:text-zinc-300 cursor-default overflow-hidden shadow-sm">
                            <span>{tag.value}</span>
                            <button onclick={() => removeTagFromEditor(i)} class="absolute right-1 opacity-0 group-hover:opacity-100 flex h-4 w-4 items-center justify-center rounded-full bg-zinc-200 text-zinc-600 hover:bg-red-500 hover:text-white dark:bg-zinc-700 dark:text-zinc-300 dark:hover:bg-red-600 transition-all cursor-pointer"><X size={10} /></button>
                        </div>
                    {/each}
                    <div class="relative tag-dropdown-container">
                        <button onclick={(e) => { e.stopPropagation(); isTagDropdownOpen = !isTagDropdownOpen; }} class="flex h-6 w-6 items-center justify-center rounded-full border border-dashed border-zinc-300 text-zinc-400 hover:border-zinc-500 hover:text-zinc-600 dark:border-zinc-600 dark:hover:border-zinc-400 dark:hover:text-zinc-300 transition-colors cursor-pointer"><Plus size={14} /></button>
                        {#if isTagDropdownOpen}
                            <div onclick={(e) => e.stopPropagation()} class="absolute left-0 top-full mt-2 w-72 flex-col rounded-lg border border-zinc-200 bg-white p-2 shadow-2xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                                <input type="text" bind:value={tagSearchQuery} placeholder="Search or create tag..." class="w-full rounded-md border border-zinc-200 bg-zinc-50 px-2 py-1.5 text-xs focus:border-emerald-500 focus:outline-none dark:border-zinc-700 dark:bg-zinc-900 dark:text-white transition-colors mb-2" />
                                {#if tagSearchQuery.trim() !== '' && !allAvailableTags.some(t => t.value.toLowerCase() === tagSearchQuery.toLowerCase())}
                                    <button onclick={createNewTag} class="w-full mb-2 flex items-center justify-center gap-1.5 rounded-md bg-emerald-50 text-emerald-600 px-2 py-1.5 text-xs font-bold hover:bg-emerald-100 dark:bg-emerald-900/20 dark:text-emerald-400 dark:hover:bg-emerald-900/40 transition-colors cursor-pointer"><Plus size={12} /> Create global Tag "{tagSearchQuery}"</button>
                                {/if}
                                <div class="max-h-60 overflow-y-auto no-scrollbar flex flex-col gap-0.5 border-t border-zinc-100 dark:border-zinc-800/50 pt-2">
                                    {#each filteredTagsForEditor as tag}
                                        <button
                                                onclick={() => addTagToEditor(tag.category, tag.value)}
                                                class="w-full text-left flex items-center justify-between rounded-md px-2 py-1.5 text-xs font-medium text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800 cursor-pointer transition-colors"
                                        >
                                            <span class="truncate pr-2">{tag.value}</span>
                                            <span class="text-[8px] uppercase text-zinc-400 font-bold tracking-wider shrink-0">{tag.category}</span>
                                        </button>
                                    {/each}
                                </div>
                            </div>
                        {/if}
                    </div>
                </div>
            </div>
        </div>
        <div class="mt-8 flex justify-end gap-3">
            <button onclick={() => editingSample = null} class="px-4 py-2 text-sm font-semibold text-zinc-600 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100 transition-colors cursor-pointer">Cancel</button>
            <button onclick={saveMetadata} class="rounded-md bg-zinc-900 px-5 py-2 text-sm font-semibold text-white shadow-sm hover:bg-zinc-800 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-white transition-colors cursor-pointer">Save Changes</button>
        </div>
    </div>
</div>