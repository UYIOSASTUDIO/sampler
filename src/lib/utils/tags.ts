// src/lib/utils/tags.ts
// src/lib/utils/tags.ts
export const BASE_TAGS = [
    { category: 'Drums', value: 'Drums' },
    { category: 'Drums', value: 'Kick' },
    { category: 'Drums', value: 'Snare' },
    { category: 'Percussion', value: 'Percussion' },
    { category: 'Synth', value: 'Synth' },
    { category: 'Bass', value: '808' },
    { category: 'Genre', value: 'Trap' },
    { category: 'Genre', value: 'Afrobeats' },
    { category: 'Genre', value: 'House' },
    { category: 'Format', value: 'One-Shot' },
    { category: 'Format', value: 'Loop' },
];

export function parseTags(tagsJson: string): Array<{category: string, value: string}> {
    try {
        return JSON.parse(tagsJson);
    } catch {
        return [];
    }
}