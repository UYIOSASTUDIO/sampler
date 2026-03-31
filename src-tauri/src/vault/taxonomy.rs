use aho_corasick::AhoCorasick;
use std::sync::OnceLock;
use std::collections::HashSet;
use std::path::Path;

static TAXONOMY_ENGINE: OnceLock<TaxonomyEngine> = OnceLock::new();

struct Rule {
    keyword: &'static str,
    category: &'static str,
    value: &'static str,
}

pub struct TaxonomyEngine {
    ac: AhoCorasick,
    rules: Vec<Rule>,
}

impl TaxonomyEngine {
    fn new() -> Self {
        let rules = vec![
            // Drums & Percussion
            Rule { keyword: "kick", category: "Drums", value: "Kick" },
            Rule { keyword: "808", category: "Bass", value: "808" },
            Rule { keyword: "snare", category: "Drums", value: "Snare" },
            Rule { keyword: "clap", category: "Drums", value: "Clap" },
            Rule { keyword: "hat", category: "Drums", value: "Hi-Hat" },
            Rule { keyword: "hihat", category: "Drums", value: "Hi-Hat" },
            Rule { keyword: "cymbal", category: "Drums", value: "Cymbal" },
            Rule { keyword: "crash", category: "Drums", value: "Crash" },
            Rule { keyword: "ride", category: "Drums", value: "Ride" },
            Rule { keyword: "tom", category: "Drums", value: "Tom" },
            Rule { keyword: "rim", category: "Drums", value: "Rimshot" },
            Rule { keyword: "shaker", category: "Percussion", value: "Shaker" },
            Rule { keyword: "tambourine", category: "Percussion", value: "Tambourine" },
            Rule { keyword: "bongo", category: "Percussion", value: "Bongo" },
            Rule { keyword: "conga", category: "Percussion", value: "Conga" },
            Rule { keyword: "perc", category: "Percussion", value: "Percussion" },

            // Instruments & Synths
            Rule { keyword: "bass", category: "Bass", value: "Bass" },
            Rule { keyword: "sub", category: "Bass", value: "Sub Bass" },
            Rule { keyword: "synth", category: "Synth", value: "Synth" },
            Rule { keyword: "pad", category: "Synth", value: "Pad" },
            Rule { keyword: "lead", category: "Synth", value: "Lead" },
            Rule { keyword: "arp", category: "Synth", value: "Arp" },
            Rule { keyword: "chord", category: "Keys", value: "Chord" },
            Rule { keyword: "piano", category: "Keys", value: "Piano" },
            Rule { keyword: "keys", category: "Keys", value: "Keys" },
            Rule { keyword: "guitar", category: "Guitar", value: "Guitar" },
            Rule { keyword: "brass", category: "Brass and Woodwinds", value: "Brass" },
            Rule { keyword: "string", category: "Strings", value: "Strings" },
            Rule { keyword: "vocal", category: "Vocals", value: "Vocals" },
            Rule { keyword: "vox", category: "Vocals", value: "Vocals" },
            Rule { keyword: "choir", category: "Vocals", value: "Choir" },

            // FX & Foley
            Rule { keyword: "fx", category: "FX", value: "FX" },
            Rule { keyword: "riser", category: "FX", value: "Riser" },
            Rule { keyword: "impact", category: "FX", value: "Impact" },
            Rule { keyword: "sweep", category: "FX", value: "Sweep" },
            Rule { keyword: "foley", category: "FX", value: "Foley" },
            Rule { keyword: "noise", category: "FX", value: "Noise" },

            // Genres
            Rule { keyword: "trap", category: "Genre", value: "Trap" },
            Rule { keyword: "hiphop", category: "Genre", value: "Hip Hop" },
            Rule { keyword: "hip hop", category: "Genre", value: "Hip Hop" },
            Rule { keyword: "house", category: "Genre", value: "House" },
            Rule { keyword: "techno", category: "Genre", value: "Techno" },
            Rule { keyword: "drill", category: "Genre", value: "Drill" },
            Rule { keyword: "lofi", category: "Genre", value: "Lo-Fi" },
            Rule { keyword: "lo-fi", category: "Genre", value: "Lo-Fi" },
            Rule { keyword: "cinematic", category: "Genre", value: "Cinematic" },
            Rule { keyword: "edm", category: "Genre", value: "EDM" },
            Rule { keyword: "dubstep", category: "Genre", value: "Dubstep" },
            Rule { keyword: "dnb", category: "Genre", value: "Drum and Bass" },
            Rule { keyword: "amapiano", category: "Genre", value: "Amapiano" },

            // Characteristics
            Rule { keyword: "dry", category: "Character", value: "Dry" },
            Rule { keyword: "wet", category: "Character", value: "Wet" },
            Rule { keyword: "distorted", category: "Character", value: "Distorted" },
            Rule { keyword: "clean", category: "Character", value: "Clean" },
            Rule { keyword: "vintage", category: "Character", value: "Vintage" },
            Rule { keyword: "acoustic", category: "Character", value: "Acoustic" },
            Rule { keyword: "punchy", category: "Character", value: "Punchy" },
            Rule { keyword: "dark", category: "Character", value: "Dark" },
            Rule { keyword: "bright", category: "Character", value: "Bright" },
            Rule { keyword: "analog", category: "Character", value: "Analog" },
        ];

        let keywords: Vec<String> = rules.iter().map(|r| r.keyword.to_string()).collect();
        let ac = AhoCorasick::builder()
            .ascii_case_insensitive(true)
            .build(keywords)
            .expect("Failed to build taxonomy engine");

        Self { ac, rules }
    }

    pub fn global() -> &'static Self {
        TAXONOMY_ENGINE.get_or_init(TaxonomyEngine::new)
    }

    pub fn analyze(&self, path: &Path, duration_ms: i64) -> Vec<serde_json::Value> {
        let mut found_tags = HashSet::new();
        let mut categories_found = HashSet::new();

        let path_str = path.to_string_lossy().to_lowercase();

        let is_cymbal_or_fx = path_str.contains("crash") || path_str.contains("ride") ||
            path_str.contains("cymbal") || path_str.contains("fx") ||
            path_str.contains("impact") || path_str.contains("riser");

        let is_explicit_loop = path_str.contains("loop") || path_str.contains("break");

        // Format determination (Länge + Heuristik)
        let format = if is_explicit_loop {
            "Loop"
        } else if duration_ms < 3500 {
            "One-Shot"
        } else if is_cymbal_or_fx {
            "One-Shot"
        } else {
            "Loop"
        };

        found_tags.insert(serde_json::json!({
            "category": "Format",
            "value": format
        }));

        for mat in self.ac.find_iter(&path_str) {
            let rule = &self.rules[mat.pattern()];

            let tag_json = serde_json::json!({
                "category": rule.category,
                "value": rule.value
            });

            if found_tags.insert(tag_json) {
                categories_found.insert(rule.category);
            }
        }

        // Auto-Inference (Wenn Sub-Bass gefunden, muss es Kategorie Bass sein)
        if path_str.contains("808") {
            found_tags.insert(serde_json::json!({ "category": "Bass", "value": "Bass" }));
            categories_found.insert("Bass");
        }

        let mut tags_vec: Vec<serde_json::Value> = found_tags.into_iter().collect();
        tags_vec.sort_by(|a, b| {
            let cat_a = a["category"].as_str().unwrap_or("");
            let cat_b = b["category"].as_str().unwrap_or("");
            cat_a.cmp(cat_b)
        });

        tags_vec
    }
}