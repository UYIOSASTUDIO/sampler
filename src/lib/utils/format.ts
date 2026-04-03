// src/lib/utils/format.ts
export function formatDuration(ms: number): string {
    if (ms === 0) return "--:--";
    const totalSec = Math.floor(ms / 1000);
    return `${Math.floor(totalSec / 60)}:${(totalSec % 60).toString().padStart(2, '0')}`;
}

export function parseWaveform(data: number[] | null, targetLength: number = 40): number[] {
    if (!data || data.length === 0) return Array(targetLength).fill(10);
    if (data.length === targetLength) return data;

    const result = [];
    const step = data.length / targetLength;
    for (let i = 0; i < targetLength; i++) {
        const idx = Math.floor(i * step);
        result.push(data[Math.min(idx, data.length - 1)] || 10);
    }
    return result;
}