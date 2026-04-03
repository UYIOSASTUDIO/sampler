// src/lib/utils/audio.ts
export const CHROMATIC_SCALE = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];

export function parseKeyMode(keyStr: string): 'min' | 'maj' {
    const lower = keyStr.toLowerCase().trim();
    if (/\d+a$/.test(lower)) return 'min';
    if (/\d+b$/.test(lower)) return 'maj';
    if (lower.includes('min')) return 'min';
    return 'maj';
}

export function parseKeyNote(keyStr: string): string {
    const camelotMatch = keyStr.match(/^(1[0-2]|[1-9])([AB])$/i);
    if (camelotMatch) {
        const CAMELOT_TO_NOTE: Record<string, string> = {
            '1A':'Ab','1B':'B','2A':'Eb','2B':'F#','3A':'Bb','3B':'Db',
            '4A':'F','4B':'Ab','5A':'C','5B':'Eb','6A':'G','6B':'Bb',
            '7A':'D','7B':'F','8A':'A','8B':'C','9A':'E','9B':'G',
            '10A':'B','10B':'D','11A':'F#','11B':'A','12A':'Db','12B':'E'
        };
        const key = camelotMatch[1].toUpperCase() + camelotMatch[2].toUpperCase();
        return CAMELOT_TO_NOTE[key] ?? '';
    }
    return keyStr.trim().split(/\s+/)[0].toUpperCase();
}

export function getSemitoneShift(
    sampleKey: string | null,
    targetNote: string | null,
    targetMode: 'min' | 'maj'
): number {
    if (!sampleKey || !targetNote) return 0;
    const RELATIVE_SEMITONES = 3;

    const sNote = parseKeyNote(sampleKey);
    const sMode = parseKeyMode(sampleKey);
    const sIdx = CHROMATIC_SCALE.indexOf(sNote);
    let tIdx  = CHROMATIC_SCALE.indexOf(targetNote.toUpperCase());

    if (sIdx === -1 || tIdx === -1) return 0;

    if (targetMode === 'min' && sMode === 'maj') {
        tIdx = (tIdx + RELATIVE_SEMITONES) % 12;
    } else if (targetMode === 'maj' && sMode === 'min') {
        tIdx = (tIdx - RELATIVE_SEMITONES + 12) % 12;
    }

    let diff = tIdx - sIdx;
    if (diff > 6)  diff -= 12;
    if (diff < -6) diff += 12;
    return diff;
}

export function getStretchRatio(sampleBpm: number | null, globalBpm: number | null): number {
    if (!globalBpm || !sampleBpm || sampleBpm <= 0) return 1.0;
    return globalBpm / sampleBpm;
}