// src/lib/types/index.ts

export type SampleRecord = {
    id: string;
    filename: string;
    original_path: string;
    duration_ms: number;
    bpm: number | null;
    key_signature: string | null;
    instrument_type: string | null;
    waveform_data: number[] | null;
    tags: string;
    is_liked: boolean;
    cover_path: string | null;
};

export type PaginatedResponse = {
    samples: SampleRecord[];
    total_count: number;
    available_tags: Array<{category: string, value: string}>;
};

export type ScanProgressPayload = {
    total: number;
    current: number;
    current_file: string;
};