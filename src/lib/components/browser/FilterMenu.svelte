<script lang="ts">
    import { appState } from '$lib/store.svelte';
    import { ChevronDown, Music2, Gauge } from 'lucide-svelte';

    let {
        activeDropdownTags,
        sortedAvailableTags,
        availableTags,
        loadSamples
    }: {
        activeDropdownTags: Array<{category: string, value: string}>,
        sortedAvailableTags: Array<{category: string, value: string}>,
        availableTags: Array<{category: string, value: string}>,
        loadSamples: () => void
    } = $props();

    let openDropdown: 'instrument' | 'genre' | 'key' | 'bpm' | 'format' | 'globalkey' | 'globalbpm' | null = $state(null);
    let isTagsExpanded: boolean = $state(false);

    const whiteKeys = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
    const blackKeys = [
        { note: 'C#', left: '14.28%' }, { note: 'D#', left: '28.56%' }, { note: 'F#', left: '57.12%' }, { note: 'G#', left: '71.40%' }, { note: 'A#', left: '85.68%' }
    ];

    let activeKeyFilter = $derived(appState.filters.keys.length > 0 ? appState.filters.keys[0] : null);
    let currentKeyMode = $derived.by(() => {
        if (!activeKeyFilter) return null;
        if (activeKeyFilter === 'min' || activeKeyFilter.endsWith(' min')) return 'min';
        if (activeKeyFilter === 'maj' || activeKeyFilter.endsWith(' maj')) return 'maj';
        return null;
    });
    let currentBaseNote = $derived.by(() => {
        if (!activeKeyFilter) return null;
        if (activeKeyFilter === 'min' || activeKeyFilter === 'maj') return null;
        return activeKeyFilter.split(' ')[0];
    });

    function togglePianoKey(note: string) {
        if (currentBaseNote === note) {
            if (currentKeyMode) appState.filters.keys = [currentKeyMode];
            else appState.filters.keys = [];
        } else {
            if (currentKeyMode) appState.filters.keys = [`${note} ${currentKeyMode}`];
            else appState.filters.keys = [note];
        }
        loadSamples();
    }

    function switchKeyMode(mode: 'min' | 'maj') {
        if (currentKeyMode === mode) {
            if (currentBaseNote) appState.filters.keys = [currentBaseNote];
            else appState.filters.keys = [];
        } else {
            if (currentBaseNote) appState.filters.keys = [`${currentBaseNote} ${mode}`];
            else appState.filters.keys = [mode];
        }
        loadSamples();
    }

    function isPianoKeyActive(note: string) { return currentBaseNote === note; }

    function setGlobalPianoKey(note: string) {
        if (appState.globalKey === note) appState.globalKey = null;
        else appState.globalKey = note;
    }

    function isGlobalKeyActive(note: string) { return appState.globalKey === note; }

    let globalKeyLabel = $derived(appState.globalKey ? `${appState.globalKey} ${appState.globalKeyMode}` : 'Off');
    let globalBpmLabel = $derived(appState.globalBpm ? `${appState.globalBpm} BPM` : 'BPM');

    function toggleDropdown(dropdown: 'instrument' | 'genre' | 'key' | 'bpm' | 'format' | 'globalkey' | 'globalbpm', event: Event) {
        event.stopPropagation();
        openDropdown = openDropdown === dropdown ? null : dropdown;
    }

    function toggleFilterTag(category: string, value: string) {
        if (category === 'Format') {
            if (appState.filters.formats.includes(value)) appState.filters.formats = [];
            else appState.filters.formats = [value];
        } else {
            let targetArray: string[];
            if (category === 'Genre') targetArray = appState.filters.genres;
            else if (category === 'Key') targetArray = appState.filters.keys;
            else targetArray = appState.filters.instruments;

            const idx = targetArray.indexOf(value);
            if (idx > -1) targetArray.splice(idx, 1);
            else targetArray.push(value);

            if (category === 'Genre') appState.filters.genres = [...targetArray];
            else if (category === 'Key') appState.filters.keys = [...targetArray];
            else appState.filters.instruments = [...targetArray];
        }
        loadSamples();
    }

    function isTagActive(category: string, value: string): boolean {
        if (category === 'Genre') return appState.filters.genres.includes(value);
        if (category === 'Format') return appState.filters.formats.includes(value);
        if (category === 'Key') return appState.filters.keys.includes(value);
        return appState.filters.instruments.includes(value);
    }

    function clearAllFilters() {
        appState.filters.instruments = []; appState.filters.genres = []; appState.filters.formats = []; appState.filters.keys = [];
        appState.filters.bpm.exact = null; appState.filters.bpm.min = null; appState.filters.bpm.max = null;
        appState.filters.tagMatchMode = 'AND';
        loadSamples();
    }

    function handleOutsideClick() {
        openDropdown = null;
    }
</script>

<svelte:window onclick={handleOutsideClick} />

<div class="mb-6 space-y-3">
    <div class="flex flex-wrap items-center gap-2">
        <div class="relative">
            <button onclick={(e) => toggleDropdown('instrument', e)} class="flex h-8 items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 text-xs font-semibold transition-colors cursor-pointer {appState.filters.instruments.length > 0 ? 'border-zinc-900 dark:border-zinc-100 text-zinc-900 dark:text-zinc-100' : 'text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800'}">
                Instrument {#if appState.filters.instruments.length > 0}<span class="flex h-4 w-4 items-center justify-center rounded-full bg-zinc-900 text-[9px] text-white dark:bg-zinc-100 dark:text-zinc-900">{appState.filters.instruments.length}</span>{/if}<ChevronDown size={14} class="opacity-50" />
            </button>
            <div onclick={(e) => e.stopPropagation()} class="absolute left-0 top-full mt-1 {openDropdown === 'instrument' ? 'flex' : 'hidden'} w-48 flex-col rounded-lg border border-zinc-200 bg-white p-1 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                <div class="max-h-60 overflow-y-auto no-scrollbar flex flex-col gap-0.5">
                    {#each activeDropdownTags.filter(t => !['Genre', 'Format', 'Key', 'Character'].includes(t.category)) as tag}
                        <label class="flex items-center gap-2 rounded-md px-2 py-1.5 text-xs hover:bg-zinc-100 dark:hover:bg-zinc-800 cursor-pointer"><input type="checkbox" checked={isTagActive(tag.category, tag.value)} onchange={() => toggleFilterTag(tag.category, tag.value)} class="rounded border-zinc-300 dark:border-zinc-700 accent-zinc-900 dark:accent-zinc-100 cursor-pointer"> {tag.value}</label>
                    {/each}
                </div>
            </div>
        </div>
        <div class="relative">
            <button onclick={(e) => toggleDropdown('genre', e)} class="flex h-8 items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 text-xs font-semibold transition-colors cursor-pointer {appState.filters.genres.length > 0 ? 'border-zinc-900 dark:border-zinc-100 text-zinc-900 dark:text-zinc-100' : 'text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800'}">
                Genre {#if appState.filters.genres.length > 0}<span class="flex h-4 w-4 items-center justify-center rounded-full bg-zinc-900 text-[9px] text-white dark:bg-zinc-100 dark:text-zinc-900">{appState.filters.genres.length}</span>{/if}<ChevronDown size={14} class="opacity-50" />
            </button>
            <div onclick={(e) => e.stopPropagation()} class="absolute left-0 top-full mt-1 {openDropdown === 'genre' ? 'flex' : 'hidden'} w-48 flex-col rounded-lg border border-zinc-200 bg-white p-1 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                <div class="max-h-60 overflow-y-auto no-scrollbar flex flex-col gap-0.5">
                    {#each activeDropdownTags.filter(t => t.category === 'Genre') as tag}
                        <label class="flex items-center gap-2 rounded-md px-2 py-1.5 text-xs hover:bg-zinc-100 dark:hover:bg-zinc-800 cursor-pointer"><input type="checkbox" checked={isTagActive('Genre', tag.value)} onchange={() => toggleFilterTag('Genre', tag.value)} class="rounded border-zinc-300 dark:border-zinc-700 accent-zinc-900 dark:accent-zinc-100 cursor-pointer"> {tag.value}</label>
                    {/each}
                </div>
            </div>
        </div>
        <div class="relative">
            <button onclick={(e) => toggleDropdown('key', e)} class="flex h-8 items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 text-xs font-semibold transition-colors cursor-pointer {appState.filters.keys.length > 0 ? 'border-zinc-900 dark:border-zinc-100 text-zinc-900 dark:text-zinc-100' : 'text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800'}">
                Key {#if appState.filters.keys.length > 0}<span class="flex h-4 w-4 items-center justify-center rounded-full bg-zinc-900 text-[9px] text-white dark:bg-zinc-100 dark:text-zinc-900">{appState.filters.keys.length}</span>{/if}<ChevronDown size={14} class="opacity-50" />
            </button>
            <div onclick={(e) => e.stopPropagation()} class="absolute left-0 top-full mt-1 {openDropdown === 'key' ? 'flex' : 'hidden'} w-64 flex-col gap-3 rounded-lg border border-zinc-200 bg-white p-3 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                <div class="flex rounded-lg bg-zinc-100 p-1 dark:bg-zinc-800/50 border border-zinc-200 dark:border-zinc-700/50 w-full">
                    <button onclick={() => switchKeyMode('min')} class="flex-1 py-1.5 text-xs font-semibold rounded-md transition-all cursor-pointer {currentKeyMode === 'min' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">Minor</button>
                    <button onclick={() => switchKeyMode('maj')} class="flex-1 py-1.5 text-xs font-semibold rounded-md transition-all cursor-pointer {currentKeyMode === 'maj' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">Major</button>
                </div>
                <div class="relative flex w-full h-24 rounded border border-zinc-300 dark:border-zinc-700 overflow-hidden select-none">
                    {#each whiteKeys as note} <button onclick={() => togglePianoKey(note)} class="flex-1 flex items-end justify-center pb-2 text-[10px] font-bold border-r border-zinc-200 dark:border-zinc-700 last:border-0 transition-colors cursor-pointer {isPianoKeyActive(note) ? 'bg-zinc-200 dark:bg-zinc-600 text-zinc-900 dark:text-white shadow-inner' : 'bg-white dark:bg-zinc-800 text-zinc-800 dark:text-zinc-300 hover:bg-zinc-50 dark:hover:bg-zinc-700'}">{note}</button> {/each}
                    {#each blackKeys as bk} <button onclick={() => togglePianoKey(bk.note)} style="left: {bk.left}; transform: translateX(-50%);" class="absolute top-0 w-[9%] h-14 rounded-b flex items-end justify-center pb-1.5 text-[8px] font-bold transition-colors cursor-pointer z-10 {isPianoKeyActive(bk.note) ? 'bg-zinc-500 text-white shadow-inner' : 'bg-zinc-900 text-zinc-300 hover:bg-zinc-800 dark:bg-black dark:hover:bg-zinc-900'}">{bk.note}</button> {/each}
                </div>
            </div>
        </div>
        <div class="relative">
            <button onclick={(e) => toggleDropdown('bpm', e)} class="flex h-8 items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 text-xs font-semibold transition-colors cursor-pointer {(appState.filters.bpm.exact || appState.filters.bpm.min || appState.filters.bpm.max) ? 'border-zinc-900 dark:border-zinc-100 text-zinc-900 dark:text-zinc-100' : 'text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800'}">
                BPM {#if appState.filters.bpm.exact || appState.filters.bpm.min || appState.filters.bpm.max}<span class="flex h-4 w-4 items-center justify-center rounded-full bg-zinc-900 text-[9px] text-white dark:bg-zinc-100 dark:text-zinc-900">!</span>{/if}<ChevronDown size={14} class="opacity-50" />
            </button>
            <div onclick={(e) => e.stopPropagation()} class="absolute left-0 top-full mt-1 {openDropdown === 'bpm' ? 'flex' : 'hidden'} w-56 flex-col gap-3 rounded-lg border border-zinc-200 bg-white p-3 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                <div class="flex items-center justify-between border-b border-zinc-100 pb-2 dark:border-zinc-800">
                    <span class="text-xs font-semibold">Mode</span><label class="flex items-center gap-2 text-[10px] uppercase font-bold tracking-wider cursor-pointer"><input type="checkbox" bind:checked={appState.filters.bpm.isRange} class="rounded border-zinc-300 accent-zinc-900"> Range</label>
                </div>
                {#if appState.filters.bpm.isRange}
                    <div class="flex items-center gap-2"><input type="number" bind:value={appState.filters.bpm.min} placeholder="Min" class="w-full rounded-md border border-zinc-200 bg-zinc-50 px-2 py-1 text-xs outline-none focus:border-zinc-900 dark:border-zinc-700 dark:bg-zinc-900 dark:focus:border-zinc-100"><span class="text-xs text-zinc-500">-</span><input type="number" bind:value={appState.filters.bpm.max} placeholder="Max" class="w-full rounded-md border border-zinc-200 bg-zinc-50 px-2 py-1 text-xs outline-none focus:border-zinc-900 dark:border-zinc-700 dark:bg-zinc-900 dark:focus:border-zinc-100"></div>
                {:else}
                    <input type="number" bind:value={appState.filters.bpm.exact} placeholder="Exact BPM (e.g. 120)" class="w-full rounded-md border border-zinc-200 bg-zinc-50 px-2 py-1 text-xs outline-none focus:border-zinc-900 dark:border-zinc-700 dark:bg-zinc-900 dark:focus:border-zinc-100">
                {/if}
                <div class="flex gap-2 mt-1">
                    <button onclick={() => { appState.filters.bpm.exact = null; appState.filters.bpm.min = null; appState.filters.bpm.max = null; loadSamples(); openDropdown = null; }} class="w-1/3 rounded border border-zinc-200 bg-white px-2 py-1.5 text-xs text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700 cursor-pointer">Clear</button>
                    <button onclick={() => { openDropdown = null; loadSamples(); }} class="w-2/3 rounded bg-zinc-900 px-2 py-1.5 text-xs text-white hover:bg-zinc-800 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-white cursor-pointer">Apply</button>
                </div>
            </div>
        </div>

        <div class="relative">
            <button onclick={(e) => toggleDropdown('format', e)} class="flex h-8 items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 text-xs font-semibold transition-colors cursor-pointer {appState.filters.formats.length > 0 ? 'border-zinc-900 dark:border-zinc-100 text-zinc-900 dark:text-zinc-100' : 'text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800'}">
                Format {#if appState.filters.formats.length > 0}<span class="flex h-4 w-4 items-center justify-center rounded-full bg-zinc-900 text-[9px] text-white dark:bg-zinc-100 dark:text-zinc-900">{appState.filters.formats.length}</span>{/if}<ChevronDown size={14} class="opacity-50" />
            </button>
            <div onclick={(e) => e.stopPropagation()} class="absolute left-0 top-full mt-1 {openDropdown === 'format' ? 'flex' : 'hidden'} w-40 flex-col rounded-lg border border-zinc-200 bg-white p-1 shadow-xl dark:border-zinc-800 dark:bg-[#18181b] z-50">
                <div class="max-h-60 overflow-y-auto no-scrollbar flex flex-col gap-0.5">
                    {#each activeDropdownTags.filter(t => t.category === 'Format') as tag}
                        <label class="flex items-center gap-2 rounded-md px-2 py-1.5 text-xs hover:bg-zinc-100 dark:hover:bg-zinc-800 cursor-pointer"><input type="checkbox" checked={isTagActive('Format', tag.value)} onchange={() => toggleFilterTag('Format', tag.value)} class="rounded border-zinc-300 dark:border-zinc-700 accent-zinc-900 dark:accent-zinc-100 cursor-pointer"> {tag.value}</label>
                    {/each}
                </div>
            </div>
        </div>

        {#if appState.filters.instruments.length > 0 || appState.filters.genres.length > 0 || appState.filters.formats.length > 0 || appState.filters.keys.length > 0 || appState.filters.bpm.exact || appState.filters.bpm.min || appState.filters.bpm.max}
            <button onclick={clearAllFilters} class="ml-2 flex h-8 items-center text-xs font-semibold text-red-600 hover:text-red-700 dark:text-red-400 dark:hover:text-red-300 transition-colors cursor-pointer">Clear all</button>
        {/if}

        <div class="mx-1 h-5 w-px bg-zinc-300 dark:bg-zinc-700 shrink-0"></div>

        <div class="relative shrink-0">
            <button
                    onclick={(e) => { e.stopPropagation(); openDropdown = openDropdown === 'globalkey' ? null : 'globalkey'; }}
                    class="flex h-8 items-center gap-1.5 rounded-full border px-3 text-xs font-bold transition-all cursor-pointer
                       {appState.globalKey
                           ? 'border-emerald-500 bg-emerald-500 text-white shadow-sm shadow-emerald-500/30 dark:shadow-emerald-500/20'
                           : 'border-emerald-500/40 bg-emerald-50 text-emerald-700 hover:border-emerald-500 hover:bg-emerald-100 dark:border-emerald-500/20 dark:bg-emerald-500/5 dark:text-emerald-400 dark:hover:border-emerald-500/50 dark:hover:bg-emerald-500/10'}"
            >
                <Music2 size={13} class="shrink-0 {appState.globalKey ? 'opacity-100' : 'opacity-70'}" />
                <span>{globalKeyLabel}</span>
                {#if !appState.globalKey}
                    <ChevronDown size={12} class="opacity-50" />
                {:else}
                    <span class="relative flex h-1.5 w-1.5 shrink-0">
                        <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-white opacity-75"></span>
                        <span class="relative inline-flex rounded-full h-1.5 w-1.5 bg-white"></span>
                    </span>
                {/if}
            </button>

            {#if openDropdown === 'globalkey'}
                <div
                        onclick={(e) => e.stopPropagation()}
                        class="absolute right-0 top-full mt-2 flex w-72 flex-col gap-3 rounded-xl border border-zinc-200 bg-white p-3 shadow-2xl dark:border-zinc-800 dark:bg-[#18181b] z-50"
                >
                    <div class="flex items-center gap-2 pb-1 border-b border-zinc-100 dark:border-zinc-800">
                        <Music2 size={13} class="text-emerald-500 shrink-0" />
                        <span class="text-[11px] font-bold uppercase tracking-wider text-zinc-500 dark:text-zinc-400">Auto-Pitch Key</span>
                    </div>

                    <div class="flex rounded-lg bg-zinc-100 p-1 dark:bg-zinc-800/50 border border-zinc-200 dark:border-zinc-700/50">
                        <button
                                onclick={() => { appState.globalKeyMode = 'min'; }}
                                class="flex-1 py-1.5 text-xs font-semibold rounded-md transition-all cursor-pointer
                                   {appState.globalKeyMode === 'min'
                                       ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white'
                                       : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}"
                        >Minor</button>
                        <button
                                onclick={() => { appState.globalKeyMode = 'maj'; }}
                                class="flex-1 py-1.5 text-xs font-semibold rounded-md transition-all cursor-pointer
                                   {appState.globalKeyMode === 'maj'
                                       ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white'
                                       : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}"
                        >Major</button>
                    </div>

                    <div class="relative flex w-full h-24 rounded-lg border border-zinc-300 dark:border-zinc-700 overflow-hidden select-none">
                        {#each whiteKeys as note}
                            <button
                                    onclick={() => setGlobalPianoKey(note)}
                                    class="flex-1 flex items-end justify-center pb-2 text-[10px] font-bold border-r border-zinc-200 dark:border-zinc-700 last:border-0 transition-colors cursor-pointer
                                       {isGlobalKeyActive(note)
                                           ? 'bg-emerald-100 dark:bg-emerald-900/40 text-emerald-700 dark:text-emerald-300'
                                           : 'bg-white dark:bg-zinc-800 text-zinc-400 hover:bg-zinc-50 dark:hover:bg-zinc-700'}"
                            >{note}</button>
                        {/each}
                        {#each blackKeys as bk}
                            <button
                                    onclick={() => setGlobalPianoKey(bk.note)}
                                    style="left: {bk.left}; transform: translateX(-50%);"
                                    class="absolute top-0 w-[9%] h-14 rounded-b flex items-end justify-center pb-1.5 text-[8px] font-bold transition-colors cursor-pointer z-10
                                       {isGlobalKeyActive(bk.note)
                                           ? 'bg-emerald-600 text-white shadow-inner'
                                           : 'bg-zinc-900 text-zinc-300 hover:bg-zinc-700 dark:bg-black dark:hover:bg-zinc-900'}"
                            >{bk.note}</button>
                        {/each}
                    </div>

                    {#if appState.globalKey}
                        <p class="text-[10px] text-zinc-400 dark:text-zinc-500 leading-relaxed">
                            Samples in <span class="font-semibold text-emerald-600 dark:text-emerald-400">{globalKeyLabel}</span> werden automatisch gepitched.
                        </p>
                    {/if}

                    <button
                            onclick={() => { appState.globalKey = null; openDropdown = null; }}
                            class="w-full rounded-lg py-1.5 text-xs font-semibold text-zinc-400 hover:bg-zinc-100 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-300 transition-colors cursor-pointer border border-transparent hover:border-zinc-200 dark:hover:border-zinc-700"
                    >Turn Off</button>
                </div>
            {/if}
        </div>

        <div class="mx-1 h-5 w-px bg-zinc-300 dark:bg-zinc-700 shrink-0"></div>

        <div class="relative shrink-0">
            <button
                    onclick={(e) => { e.stopPropagation(); openDropdown = openDropdown === 'globalbpm' ? null : 'globalbpm'; }}
                    class="flex h-8 items-center gap-1.5 rounded-full border px-3 text-xs font-bold transition-all cursor-pointer
                       {appState.globalBpm
                           ? 'border-violet-500 bg-violet-500 text-white shadow-sm shadow-violet-500/30 dark:shadow-violet-500/20'
                           : 'border-violet-500/40 bg-violet-50 text-violet-700 hover:border-violet-500 hover:bg-violet-100 dark:border-violet-500/20 dark:bg-violet-500/5 dark:text-violet-400 dark:hover:border-violet-500/50 dark:hover:bg-violet-500/10'}"
            >
                <Gauge size={13} class="shrink-0 {appState.globalBpm ? 'opacity-100' : 'opacity-70'}" />
                <span>{globalBpmLabel}</span>
                {#if !appState.globalBpm}
                    <ChevronDown size={12} class="opacity-50" />
                {:else}
                    <span class="relative flex h-1.5 w-1.5 shrink-0">
                        <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-white opacity-75"></span>
                        <span class="relative inline-flex rounded-full h-1.5 w-1.5 bg-white"></span>
                    </span>
                {/if}
            </button>

            {#if openDropdown === 'globalbpm'}
                <div
                        onclick={(e) => e.stopPropagation()}
                        class="absolute left-0 top-full mt-2 w-64 flex-col gap-3 rounded-xl border border-zinc-200 bg-white p-3 shadow-2xl dark:border-zinc-700/60 dark:bg-[#18181b] z-50 flex"
                >
                    <div class="flex items-center gap-2 border-b border-zinc-100 pb-2.5 dark:border-zinc-800">
                        <Gauge size={13} class="text-violet-500 shrink-0" />
                        <span class="text-xs font-bold text-zinc-700 dark:text-zinc-200">BPM Stretcher</span>
                    </div>

                    <div class="flex items-center gap-2">
                        <button
                                onclick={() => { if (appState.globalBpm && appState.globalBpm > 1) appState.globalBpm = Math.max(1, appState.globalBpm - 1); }}
                                class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-zinc-200 bg-zinc-50 text-sm font-bold text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700 transition-colors cursor-pointer select-none"
                        >−</button>
                        <input
                                type="number" min="1" max="300" placeholder="e.g. 120"
                                value={appState.globalBpm ?? ''}
                                oninput={(e) => {
                                    const v = parseInt((e.target as HTMLInputElement).value);
                                    appState.globalBpm = (!isNaN(v) && v > 0) ? v : null;
                                }}
                                class="h-8 flex-1 rounded-lg border border-zinc-200 bg-zinc-50 px-3 text-center text-sm font-bold text-zinc-900 focus:border-violet-500 focus:outline-none focus:ring-1 focus:ring-violet-500/30 dark:border-zinc-700 dark:bg-zinc-800 dark:text-white dark:focus:border-violet-500 transition-colors"
                        />
                        <button
                                onclick={() => { appState.globalBpm = (appState.globalBpm ?? 119) + 1; }}
                                class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-zinc-200 bg-zinc-50 text-sm font-bold text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700 transition-colors cursor-pointer select-none"
                        >+</button>
                    </div>

                    <div>
                        <p class="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">Presets</p>
                        <div class="grid grid-cols-5 gap-1">
                            {#each [80, 90, 100, 110, 120, 128, 140, 150, 160, 174] as preset}
                                <button
                                        onclick={() => { appState.globalBpm = preset; }}
                                        class="rounded-md py-1.5 text-xs font-semibold transition-colors cursor-pointer
                                           {appState.globalBpm === preset
                                               ? 'bg-violet-500 text-white shadow-sm'
                                               : 'bg-zinc-100 text-zinc-600 hover:bg-violet-50 hover:text-violet-700 dark:bg-zinc-800 dark:text-zinc-400 dark:hover:bg-violet-500/10 dark:hover:text-violet-300'}"
                                >{preset}</button>
                            {/each}
                        </div>
                    </div>

                    {#if !appState.globalBpm}
                        <p class="text-[10px] text-zinc-400 dark:text-zinc-500 leading-relaxed">
                            Wähle ein Ziel-BPM. Nur Samples mit bekanntem BPM werden beeinflusst.
                        </p>
                    {/if}

                    <button
                            onclick={() => { appState.globalBpm = null; openDropdown = null; }}
                            class="w-full rounded-lg py-1.5 text-xs font-semibold text-zinc-400 hover:bg-zinc-100 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-300 transition-colors cursor-pointer border border-transparent hover:border-zinc-200 dark:hover:border-zinc-700"
                    >Turn Off</button>
                </div>
            {/if}
        </div>

        <div class="ml-auto flex items-center gap-2">
            <span class="text-[10px] font-bold uppercase tracking-wider text-zinc-400">Match:</span>
            <div class="flex h-8 rounded-full border border-zinc-200 bg-zinc-50 p-[3px] dark:border-zinc-700/50 dark:bg-zinc-900">
                <button onclick={() => { appState.filters.tagMatchMode = 'OR'; loadSamples(); }} class="px-3 text-[10px] font-bold uppercase tracking-wider rounded-full transition-colors cursor-pointer {appState.filters.tagMatchMode === 'OR' ? 'bg-emerald-500 text-white shadow-sm' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">Either</button>
                <button onclick={() => { appState.filters.tagMatchMode = 'AND'; loadSamples(); }} class="px-3 text-[10px] font-bold uppercase tracking-wider rounded-full transition-colors cursor-pointer {appState.filters.tagMatchMode === 'AND' ? 'bg-emerald-500 text-white shadow-sm' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">Both</button>
            </div>
        </div>
    </div>

    <div class="relative w-full">
        <div class="flex w-full flex-wrap content-start items-start gap-2 pr-10 transition-all {isTagsExpanded ? 'h-auto pb-1' : 'h-6 overflow-hidden'}">
            {#each sortedAvailableTags as tag (tag.category + tag.value)}
                <button onclick={() => toggleFilterTag(tag.category, tag.value)} class="shrink-0 flex items-center h-6 rounded-full border px-3 text-[11px] font-semibold cursor-pointer transition-colors {isTagActive(tag.category, tag.value) ? 'border-zinc-900 bg-zinc-900 text-white dark:border-zinc-100 dark:bg-zinc-100 dark:text-zinc-900' : 'border-zinc-200 bg-zinc-50 text-zinc-600 hover:border-zinc-300 hover:bg-zinc-100 dark:border-zinc-800 dark:bg-[#18181b] dark:text-zinc-400 dark:hover:bg-zinc-800'}">{tag.value} {#if isTagActive(tag.category, tag.value)}<span class="ml-1.5 opacity-50 font-normal hover:opacity-100">✕</span>{/if}</button>
            {/each}
        </div>
        {#if sortedAvailableTags.length > 0}
            <div class="absolute right-0 top-0 h-6 bg-gradient-to-l from-white via-white to-transparent pl-8 pr-1 dark:from-[#18181b] dark:via-[#18181b]">
                <button onclick={() => isTagsExpanded = !isTagsExpanded} class="flex h-full items-center justify-center rounded px-1.5 text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer">{#if isTagsExpanded}<span class="text-sm font-bold leading-none mt-[1px]">✕</span>{:else}<span class="text-sm font-bold leading-none tracking-widest -mt-2">...</span>{/if}</button>
            </div>
        {/if}
    </div>
</div>