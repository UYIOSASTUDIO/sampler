<script lang="ts">
    import { appState } from '$lib/store.svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { ask } from '@tauri-apps/plugin-dialog';
    import { RefreshCw } from 'lucide-svelte';
    import { onMount } from 'svelte';
    import type { SampleRecord } from '$lib/types';
    import { parseTags } from '$lib/utils/tags';

    let {
        samples = $bindable(),
        allAvailableTags,
        loadAllTags
    }: {
        samples: SampleRecord[],
        allAvailableTags: Array<{category: string, value: string}>,
        loadAllTags: () => Promise<void>
    } = $props();

    let connectedFolders: string[] = $state([]);
    let isSettingsLoading: boolean = $state(false);
    let isClearing: boolean = $state(false);
    let scanMessage: string = $state('');

    onMount(() => {
        loadConnectedFolders();
    });

    function setThemePref(pref: 'light' | 'dark' | 'system') {
        appState.themePreference = pref;
        localStorage.setItem('samplevault-theme', pref);
    }

    async function loadConnectedFolders() {
        isSettingsLoading = true;
        try { connectedFolders = await invoke<string[]>('get_connected_folders'); }
        catch (error) { console.error(error); }
        finally { isSettingsLoading = false; }
    }

    async function handleRemoveFolder(folderPath: string) {
        const confirmed = await ask(`Un-link this folder?\n\n${folderPath}\n\nThis will remove all its samples from your library.`, {
            title: 'SampleVault',
            kind: 'warning'
        });
        if (!confirmed) return;

        isSettingsLoading = true;
        try {
            await invoke('remove_folder', { path: folderPath });
            await loadConnectedFolders();
            if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent('trigger-sample-reload'));
        } catch (error) { console.error(error); }
        finally { isSettingsLoading = false; }
    }

    async function handleClearDatabase() {
        const confirmed = await ask("Clear the entire library? This action cannot be undone.", {
            title: 'SampleVault',
            kind: 'warning'
        });

        if (confirmed) {
            isClearing = true;
            try {
                if (appState.isPlaying) {
                    invoke('stop_audio').catch(console.error);
                    appState.isPlaying = false;
                }

                appState.currentSample = null;
                await invoke('clear_database');

                samples = [];
                appState.collections = [];
                appState.filters.collectionId = null;
                connectedFolders = [];

                scanMessage = 'Library cleared.';
                if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent('trigger-sample-reload'));
            } catch (error) { scanMessage = `Error: ${error}`; }
            finally { isClearing = false; }
        }
    }

    async function handleDeleteUserTag(value: string, event: Event) {
        event.stopPropagation();
        try {
            await invoke('delete_user_tag', { value });
            await loadAllTags();

            samples = samples.map(sample => {
                let parsed = parseTags(sample.tags);
                let filtered = parsed.filter((t: {category: string, value: string}) => t.value !== value);

                if (parsed.length !== filtered.length) {
                    sample.tags = JSON.stringify(filtered);
                }
                return sample;
            });
        } catch (e) {
            console.error(e);
        }
    }
</script>

<div class="flex h-full w-full flex-col overflow-y-auto px-10 py-8">
    <div class="flex items-center justify-between mb-8">
        <h1 class="text-3xl font-bold tracking-tight">Preferences</h1>
        {#if scanMessage}
            <span class="text-xs font-medium text-emerald-500 animate-pulse">{scanMessage}</span>
        {/if}
    </div>

    <div class="max-w-3xl">
        {#if appState.activeSettingsTab === 'general'}
            <div class="space-y-4">
                <h3 class="text-xs font-bold uppercase tracking-wider text-zinc-400 border-b border-zinc-200 dark:border-zinc-800/50 pb-2 mb-6">Appearance</h3>
                <label class="text-sm font-medium text-zinc-900 dark:text-zinc-100 block">Theme Preference</label>
                <div class="flex rounded-lg bg-zinc-100 p-1 dark:bg-zinc-800/50 border border-zinc-200 dark:border-zinc-700/50 w-fit">
                    <button onclick={() => setThemePref('light')} class="px-6 py-2 text-xs font-semibold rounded-md transition-all cursor-pointer {appState.themePreference === 'light' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">Light</button>
                    <button onclick={() => setThemePref('dark')} class="px-6 py-2 text-xs font-semibold rounded-md transition-all cursor-pointer {appState.themePreference === 'dark' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">Dark</button>
                    <button onclick={() => setThemePref('system')} class="px-6 py-2 text-xs font-semibold rounded-md transition-all cursor-pointer {appState.themePreference === 'system' ? 'bg-white text-zinc-900 shadow-sm dark:bg-[#1f1f22] dark:text-white' : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'}">System</button>
                </div>
                <p class="text-xs text-zinc-500 pt-2">"System" automatically matches your Mac's appearance settings.</p>
            </div>

            <h3 class="text-xs font-bold uppercase tracking-wider text-zinc-400 border-b border-zinc-200 dark:border-zinc-800/50 pb-2 mb-6 mt-10">Playback</h3>

            <div class="flex items-center justify-between">
                <div>
                    <label class="text-sm font-medium text-zinc-900 dark:text-zinc-100 block">Auto-Play Selection</label>
                    <p class="text-xs text-zinc-500 mt-1">Automatically play sounds when navigating the list with arrow keys.</p>
                </div>
                <label class="relative inline-flex items-center cursor-pointer">
                    <input type="checkbox" bind:checked={appState.autoPlayEnabled} class="sr-only peer">
                    <div class="w-11 h-6 bg-zinc-200 peer-focus:outline-none rounded-full peer dark:bg-zinc-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-zinc-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-zinc-600 peer-checked:bg-emerald-500"></div>
                </label>
            </div>
        {:else if appState.activeSettingsTab === 'library'}
            <div>
                <h3 class="text-xs font-bold uppercase tracking-wider text-zinc-400 border-b border-zinc-200 dark:border-zinc-800/50 pb-2 mb-6">Connected Folders</h3>
                {#if isSettingsLoading}
                    <div class="flex h-20 items-center justify-center text-sm text-zinc-500 animate-pulse">Loading library data...</div>
                {:else if connectedFolders.length === 0}
                    <div class="flex h-20 items-center justify-center rounded-md border border-dashed border-zinc-200 text-sm text-zinc-500 dark:border-zinc-800">No folders connected yet.</div>
                {:else}
                    <div class="space-y-2">
                        {#each connectedFolders as folder}
                            <div class="flex items-center justify-between rounded-md border border-zinc-200 bg-white p-3 shadow-sm dark:border-zinc-800 dark:bg-zinc-900/50">
                                <div class="flex flex-col overflow-hidden pr-4"><span class="truncate text-sm font-medium text-zinc-700 dark:text-zinc-300" title={folder}>{folder}</span></div>
                                <button onclick={() => handleRemoveFolder(folder)} disabled={isSettingsLoading} class="shrink-0 rounded-md border border-red-200 bg-red-50 px-3 py-1.5 text-xs font-medium text-red-700 hover:bg-red-100 transition-colors dark:border-red-900/30 dark:bg-red-900/10 dark:text-red-400 dark:hover:bg-red-900/20 cursor-pointer disabled:opacity-50">Un-link</button>
                            </div>
                        {/each}
                    </div>
                {/if}
                <p class="mt-4 text-xs text-zinc-500">Un-linking a folder removes all its indexed samples from this application. It does not delete the actual files from your computer.</p>
                <div class="mt-12 border-t border-zinc-200 pt-8 dark:border-zinc-800/50">
                    <h3 class="mb-4 text-xs font-bold uppercase tracking-wider text-red-500">Danger Zone</h3>
                    <div class="flex items-center justify-between rounded-md border border-red-200 bg-red-50 p-4 dark:border-red-900/30 dark:bg-red-900/10">
                        <div class="flex flex-col pr-4">
                            <span class="text-sm font-bold text-red-700 dark:text-red-400">Clear Entire Library</span>
                            <span class="text-xs text-red-600/80 dark:text-red-400/80 mt-1">This will instantly wipe all indexed samples and collections from your database. Your actual audio files on the hard drive remain untouched.</span>
                        </div>
                        <button onclick={handleClearDatabase} disabled={isSettingsLoading} class="shrink-0 rounded-md bg-red-600 px-4 py-2 text-xs font-bold text-white transition-colors hover:bg-red-700 disabled:opacity-50 cursor-pointer shadow-sm">
                            {#if isClearing} <RefreshCw size={14} class="animate-spin inline mr-1" /> Clearing... {:else} Clear Database {/if}
                        </button>
                    </div>
                </div>
            </div>
        {:else if appState.activeSettingsTab === 'audio'}
            <div>
                <h3 class="text-xs font-bold uppercase tracking-wider text-zinc-400 border-b border-zinc-200 dark:border-zinc-800/50 pb-2 mb-6">Audio Engine</h3>
                <div class="flex h-32 items-center justify-center rounded-md border border-dashed border-zinc-200 text-sm text-zinc-500 dark:border-zinc-800">Audio Device Routing Options (Coming Soon)</div>
            </div>
        {:else if appState.activeSettingsTab === 'tags'}
            <div>
                <h3 class="text-xs font-bold uppercase tracking-wider text-zinc-400 border-b border-zinc-200 dark:border-zinc-800/50 pb-2 mb-6">Tag Management</h3>
                <div class="space-y-2">
                    {#each allAvailableTags.filter(t => t.category === 'User') as tag}
                        <div class="flex items-center justify-between rounded-md border border-zinc-200 bg-white p-3 shadow-sm dark:border-zinc-800 dark:bg-zinc-900/50">
                            <div class="flex flex-col overflow-hidden pr-4">
                                <span class="truncate text-sm font-medium text-zinc-700 dark:text-zinc-300">{tag.value}</span>
                                <span class="text-[10px] uppercase text-zinc-500 font-bold tracking-wider mt-0.5">Global User Tag</span>
                            </div>
                            <button onclick={(e) => handleDeleteUserTag(tag.value, e)} class="shrink-0 rounded-md border border-red-200 bg-red-50 px-3 py-1.5 text-xs font-medium text-red-700 hover:bg-red-100 transition-colors dark:border-red-900/30 dark:bg-red-900/10 dark:text-red-400 dark:hover:bg-red-900/20 cursor-pointer shadow-sm">Delete</button>
                        </div>
                    {/each}
                    {#if allAvailableTags.filter(t => t.category === 'User').length === 0}
                        <div class="flex h-20 items-center justify-center rounded-md border border-dashed border-zinc-200 text-sm text-zinc-500 dark:border-zinc-800">No custom tags created yet.</div>
                    {/if}
                </div>
                <p class="mt-4 text-xs text-zinc-500">Deleting a tag here will automatically remove it from all samples in your entire library. This action cannot be undone.</p>
            </div>
        {/if}
    </div>
</div>