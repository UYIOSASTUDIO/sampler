export const appState = $state({
    // NEU: 'settings' als View hinzugefügt
    currentView: 'sounds' as 'sounds' | 'projects' | 'editor' | 'settings',

    // NEU: Der aktive Tab für die Sidebar
    activeSettingsTab: 'general' as 'general' | 'library' | 'audio',

    activeSoundsTab: 'samples' as 'samples' | 'collections',

    globalKey: null as string | null,
    // 'min' | 'maj' — bestimmt ob der globale Key als Minor oder Major interpretiert wird.
    // Beeinflusst die Relative-Key-Logik im Pitch-Shifter (z.B. D min → F maj für Major-Samples).
    globalKeyMode: 'min' as 'min' | 'maj',

    // BPM Stretcher — null = off, number = target BPM.
    // Nur Samples mit bekanntem BPM werden beeinflusst.
    // Tempo wird via ssstretch ohne Pitch-Änderung angepasst.
    globalBpm: null as number | null,

    isCreateCollectionModalOpen: false,
    themePreference: 'system' as 'light' | 'dark' | 'system',
    isDarkMode: true,

    globalSearchQuery: '',
    selectedSampleIds: [] as string[], // Hält alle Checkbox-IDs
    collections: [] as {id: number, name: string}[], // Hält die Ordner-Liste

    currentSample: null as any,
    isPlaying: false,
    playbackProgress: 0,
    globalVolume: 0.8,

    cmdTogglePlay: 0,
    cmdNext: 0,
    cmdPrev: 0,

    filters: {
        instruments: [] as string[],
        genres: [] as string[],
        keys: [] as string[],
        formats: [] as string[],
        bpm: {
            isRange: false,
            exact: null as number | null,
            min: null as number | null,
            max: null as number | null
        },
        tagMatchMode: 'AND' as 'AND' | 'OR', // NEU: 'AND' = Both, 'OR' = Either
        onlyLiked: false,
        collectionId: null as number | null
    }
});