// src/lib/utils/tags.ts
export function parseTags(tagsJson: string): Array<{category: string, value: string}> {
    try {
        return JSON.parse(tagsJson);
    } catch {
        return [];
    }
}