<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { Play, Pause, Heart, EllipsisVertical, Image as ImageIcon } from 'lucide-svelte';
    import { appState } from '$lib/store.svelte';
    import type { SampleRecord } from '$lib/types';
    import { formatDuration, parseWaveform } from '$lib/utils/format';
    import { parseTags } from '$lib/utils/tags';
    import { nativeDrag } from '$lib/utils/drag';

    let {
        sample,
        selectedId,
        playingId,
        openContextMenuId = $bindable(),
        handlePlayRequest,
        toggleLike,
        openEditModal,
        openSampler,
        toggleSampleSelection
    }: {
        sample: SampleRecord,
        selectedId: string | null,
        playingId: string | null,
        openContextMenuId: string | null,
        handlePlayRequest: (sample: SampleRecord) => void,
        toggleLike: (sample: SampleRecord, event: Event) => void,
        openEditModal: (sample: SampleRecord) => void,
        openSampler: (sample: SampleRecord) => void,
        toggleSampleSelection: (id: string, checked: boolean) => void
    } = $props();
</script>

<div id="sample-{sample.id}" class="group grid grid-cols-[20px_40px_32px_minmax(150px,2fr)_minmax(120px,1.5fr)_50px_40px_40px_32px_32px] items-center gap-4 py-2 rounded-md -mx-2 px-2 {selectedId === sample.id ? 'bg-zinc-100 dark:bg-zinc-800/60' : 'hover:bg-zinc-50 dark:hover:bg-zinc-800/20'}">
    <div class="flex justify-center">
        <input type="checkbox" checked={appState.selectedSampleIds.includes(sample.id)} onchange={(e) => toggleSampleSelection(sample.id, e.currentTarget.checked)} class="h-4 w-4 rounded border-zinc-300 bg-zinc-100 cursor-pointer accent-zinc-900 dark:accent-zinc-100">
    </div>

    <div
            use:nativeDrag={sample}
            class="h-10 w-10 flex items-center justify-center rounded-md bg-zinc-200/50 text-zinc-400 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700/50 overflow-hidden shrink-0 cursor-grab active:cursor-grabbing"
    >
        {#if sample.cover_path}
            <img src={convertFileSrc(sample.cover_path)} alt="Cover" class="h-full w-full object-cover pointer-events-none" loading="lazy" />
        {:else}
            <ImageIcon size={20} class="pointer-events-none" />
        {/if}
    </div>

    <div class="flex justify-center">
        <button onclick={() => handlePlayRequest(sample)} class="flex h-8 w-8 items-center justify-center rounded-full bg-zinc-900 text-zinc-100 hover:scale-105 dark:bg-zinc-100 dark:text-zinc-900 transition-transform cursor-pointer shadow-sm">
            {#if playingId === sample.id} <Pause size={14} /> {:else} <Play size={14} class="ml-0.5" /> {/if}
        </button>
    </div>

    <div class="flex flex-col min-w-0 pr-4 cursor-pointer" role="button" tabindex="0" onclick={() => { selectedId = sample.id; }} onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') selectedId = sample.id; }}>
        <span use:nativeDrag={sample} class="truncate text-sm font-semibold hover:underline cursor-grab active:cursor-grabbing select-none" title={sample.original_path}>
            {sample.filename}
        </span>
        <div class="flex flex-wrap gap-1.5 mt-1 h-4 overflow-hidden">
            {#each parseTags(sample.tags) as tag}
                <span class="rounded px-1.5 py-[1px] text-[9px] font-bold uppercase tracking-wider {tag.category === 'Format' ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400' : tag.category === 'Drums' || tag.category === 'Percussion' ? 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400' : tag.category === 'Genre' ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400' : 'bg-zinc-200/60 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400'}">{tag.value}</span>
            {/each}
            {#if parseTags(sample.tags).length === 0} <span class="rounded bg-zinc-200/60 px-1.5 py-[1px] text-[9px] font-bold uppercase tracking-wider text-zinc-400 dark:bg-zinc-800">AUDIO</span> {/if}
        </div>
    </div>

    <div use:nativeDrag={sample} class="flex items-center gap-[2px] h-8 overflow-hidden opacity-60 group-hover:opacity-100 transition-opacity cursor-grab active:cursor-grabbing">
        {#each parseWaveform(sample.waveform_data) as barHeight, i}
            <div class="w-[3px] rounded-full pointer-events-none {playingId === sample.id && (i / 40) <= appState.playbackProgress ? 'bg-emerald-500' : 'bg-zinc-300 dark:bg-zinc-700'}" style="height: {barHeight}%;"></div>
        {/each}
    </div>

    <div class="text-right text-xs font-medium text-zinc-500 tabular-nums">{formatDuration(sample.duration_ms)}</div>
    <div class="text-center text-xs font-semibold text-zinc-700 dark:text-zinc-300">{sample.key_signature || "--"}</div>
    <div class="text-center text-xs font-semibold text-zinc-700 dark:text-zinc-300">{sample.bpm ? Math.round(sample.bpm) : "--"}</div>

    <div class="flex justify-center">
        <button onclick={(e) => toggleLike(sample, e)} class="transition-colors cursor-pointer group-hover:opacity-100 {selectedId === sample.id || sample.is_liked ? 'opacity-100' : 'opacity-0'} {sample.is_liked ? 'text-red-500 hover:text-red-600' : 'text-zinc-400 hover:text-red-500'}">
            <Heart size={16} class={sample.is_liked ? 'fill-red-500' : ''} />
        </button>
    </div>

    <div class="relative flex justify-center">
        <button onclick={(e) => { e.stopPropagation(); openContextMenuId = openContextMenuId === sample.id ? null : sample.id; }} class="text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer group-hover:opacity-100 {selectedId === sample.id || openContextMenuId === sample.id ? 'opacity-100' : 'opacity-0'}">
            <EllipsisVertical size={16} />
        </button>
        {#if openContextMenuId === sample.id}
            <div onclick={(e) => e.stopPropagation()} class="absolute right-full top-0 mr-2 w-40 flex-col rounded-lg border border-zinc-200 bg-white p-1 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                <button onclick={() => { openSampler(sample); openContextMenuId = null; }} class="w-full text-left rounded-md px-3 py-2 text-xs font-medium text-emerald-600 hover:bg-emerald-50 dark:text-emerald-400 dark:hover:bg-emerald-900/30 cursor-pointer transition-colors">Open in Sampler</button>
                <div class="my-0.5 border-t border-zinc-200 dark:border-zinc-800/50"></div>
                <button onclick={() => openEditModal(sample)} class="w-full text-left rounded-md px-3 py-2 text-xs font-medium text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800 cursor-pointer transition-colors">Edit Metadata</button>
                <div class="my-0.5 border-t border-zinc-200 dark:border-zinc-800/50"></div>
                <button onclick={() => { invoke('reveal_in_finder', { path: sample.original_path }); openContextMenuId = null; }} class="w-full text-left rounded-md px-3 py-2 text-xs font-medium text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800 cursor-pointer transition-colors">Reveal in Finder</button>
            </div>
        {/if}
    </div>
</div>