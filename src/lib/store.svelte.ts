export const appState = $state({
    currentView: 'sounds' as 'sounds' | 'projects' | 'editor',
    isSettingsOpen: false,
    themePreference: 'system' as 'light' | 'dark' | 'system',
    isDarkMode: true,

    // --- GLOBAL AUDIO STATE ---
    currentSample: null as any,
    isPlaying: false,
    playbackProgress: 0,
    globalVolume: 0.8, // Standardlautstärke 80%

    // --- IPC COMMANDS (Footer -> Page) ---
    cmdTogglePlay: 0,
    cmdNext: 0,
    cmdPrev: 0,
});