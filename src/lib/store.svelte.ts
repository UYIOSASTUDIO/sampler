// src/lib/store.svelte.ts
export const appState = $state({
    currentView: 'sounds' as 'sounds' | 'projects' | 'editor',
    isSettingsOpen: false,

    // Theme Management
    themePreference: 'system' as 'light' | 'dark' | 'system',
    isDarkMode: true // Spiegelt den echten, aktuell gerenderten Zustand wider
});