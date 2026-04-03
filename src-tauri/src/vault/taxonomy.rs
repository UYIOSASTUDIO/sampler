use aho_corasick::AhoCorasick;
use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

static TAXONOMY_ENGINE: OnceLock<TaxonomyEngine> = OnceLock::new();

pub struct Rule {
    pub keyword: &'static str,
    pub category: &'static str,
    pub value: &'static str,
}

pub struct TaxonomyEngine {
    pub ac: AhoCorasick,
    pub rules: Vec<Rule>,
}

impl TaxonomyEngine {
    pub fn new() -> Self {
        let rules = vec![
            // 🥁 Drums (Kern-Elemente)
            Rule {
                keyword: "kick",
                category: "Drums",
                value: "Kick",
            },
            Rule {
                keyword: "808 kick",
                category: "Drums",
                value: "808 Kick",
            },
            Rule {
                keyword: "909 kick",
                category: "Drums",
                value: "909 Kick",
            },
            Rule {
                keyword: "snare",
                category: "Drums",
                value: "Snare",
            },
            Rule {
                keyword: "clap",
                category: "Drums",
                value: "Clap",
            },
            Rule {
                keyword: "hat",
                category: "Drums",
                value: "Hi-Hat",
            },
            Rule {
                keyword: "hihat",
                category: "Drums",
                value: "Hi-Hat",
            },
            Rule {
                keyword: "hi-hat",
                category: "Drums",
                value: "Hi-Hat",
            },
            Rule {
                keyword: "open hat",
                category: "Drums",
                value: "Open Hat",
            },
            Rule {
                keyword: "closed hat",
                category: "Drums",
                value: "Closed Hat",
            },
            Rule {
                keyword: "cymbal",
                category: "Drums",
                value: "Cymbal",
            },
            Rule {
                keyword: "crash",
                category: "Drums",
                value: "Crash",
            },
            Rule {
                keyword: "ride",
                category: "Drums",
                value: "Ride",
            },
            Rule {
                keyword: "splash",
                category: "Drums",
                value: "Splash",
            },
            Rule {
                keyword: "tom",
                category: "Drums",
                value: "Tom",
            },
            Rule {
                keyword: "rim",
                category: "Drums",
                value: "Rimshot",
            },
            Rule {
                keyword: "rimshot",
                category: "Drums",
                value: "Rimshot",
            },
            Rule {
                keyword: "drum fill",
                category: "Drums",
                value: "Fill",
            },
            Rule {
                keyword: "breakbeat",
                category: "Drums",
                value: "Break",
            },
            // 🪘 Percussion
            Rule {
                keyword: "perc",
                category: "Percussion",
                value: "Percussion",
            },
            Rule {
                keyword: "percussion",
                category: "Percussion",
                value: "Percussion",
            },
            Rule {
                keyword: "shaker",
                category: "Percussion",
                value: "Shaker",
            },
            Rule {
                keyword: "tamb",
                category: "Percussion",
                value: "Tambourine",
            },
            Rule {
                keyword: "tambourine",
                category: "Percussion",
                value: "Tambourine",
            },
            Rule {
                keyword: "bongo",
                category: "Percussion",
                value: "Bongo",
            },
            Rule {
                keyword: "conga",
                category: "Percussion",
                value: "Conga",
            },
            Rule {
                keyword: "djembe",
                category: "Percussion",
                value: "Djembe",
            },
            Rule {
                keyword: "cajon",
                category: "Percussion",
                value: "Cajon",
            },
            Rule {
                keyword: "tabla",
                category: "Percussion",
                value: "Tabla",
            },
            Rule {
                keyword: "cowbell",
                category: "Percussion",
                value: "Cowbell",
            },
            Rule {
                keyword: "triangle",
                category: "Percussion",
                value: "Triangle",
            },
            Rule {
                keyword: "tri",
                category: "Percussion",
                value: "Triangle",
            },
            Rule {
                keyword: "snap",
                category: "Percussion",
                value: "Snap",
            },
            Rule {
                keyword: "wood",
                category: "Percussion",
                value: "Wood",
            },
            Rule {
                keyword: "clave",
                category: "Percussion",
                value: "Clave",
            },
            Rule {
                keyword: "guiro",
                category: "Percussion",
                value: "Guiro",
            },
            // 🎸 Bass
            Rule {
                keyword: "bass",
                category: "Bass",
                value: "Bass",
            },
            Rule {
                keyword: "sub",
                category: "Bass",
                value: "Sub Bass",
            },
            Rule {
                keyword: "808",
                category: "Bass",
                value: "808",
            },
            Rule {
                keyword: "reese",
                category: "Bass",
                value: "Reese Bass",
            },
            Rule {
                keyword: "donk",
                category: "Bass",
                value: "Donk",
            },
            // 🎹 Synth & Keys
            Rule {
                keyword: "synth",
                category: "Synth",
                value: "Synth",
            },
            Rule {
                keyword: "pad",
                category: "Synth",
                value: "Pad",
            },
            Rule {
                keyword: "lead",
                category: "Synth",
                value: "Lead",
            },
            Rule {
                keyword: "arp",
                category: "Synth",
                value: "Arp",
            },
            Rule {
                keyword: "pluck",
                category: "Synth",
                value: "Pluck",
            },
            Rule {
                keyword: "stab",
                category: "Synth",
                value: "Stab",
            },
            Rule {
                keyword: "chord",
                category: "Keys",
                value: "Chord",
            },
            Rule {
                keyword: "piano",
                category: "Keys",
                value: "Piano",
            },
            Rule {
                keyword: "keys",
                category: "Keys",
                value: "Keys",
            },
            Rule {
                keyword: "rhodes",
                category: "Keys",
                value: "Rhodes",
            },
            // 🎻 Instrumente
            Rule {
                keyword: "guitar",
                category: "Guitar",
                value: "Guitar",
            },
            Rule {
                keyword: "acoustic guitar",
                category: "Guitar",
                value: "Acoustic Guitar",
            },
            Rule {
                keyword: "electric guitar",
                category: "Guitar",
                value: "Electric Guitar",
            },
            Rule {
                keyword: "brass",
                category: "Brass and Woodwinds",
                value: "Brass",
            },
            Rule {
                keyword: "flute",
                category: "Brass and Woodwinds",
                value: "Flute",
            },
            Rule {
                keyword: "sax",
                category: "Brass and Woodwinds",
                value: "Saxophone",
            },
            Rule {
                keyword: "string",
                category: "Strings",
                value: "Strings",
            },
            Rule {
                keyword: "violin",
                category: "Strings",
                value: "Violin",
            },
            // 🎤 Vocals
            Rule {
                keyword: "vocal",
                category: "Vocals",
                value: "Vocals",
            },
            Rule {
                keyword: "vox",
                category: "Vocals",
                value: "Vocals",
            },
            Rule {
                keyword: "choir",
                category: "Vocals",
                value: "Choir",
            },
            Rule {
                keyword: "chant",
                category: "Vocals",
                value: "Chant",
            },
            Rule {
                keyword: "phrase",
                category: "Vocals",
                value: "Phrase",
            },
            Rule {
                keyword: "acapella",
                category: "Vocals",
                value: "Acapella",
            },
            // 🌪️ FX & Foley
            Rule {
                keyword: "fx",
                category: "FX",
                value: "FX",
            },
            Rule {
                keyword: "riser",
                category: "FX",
                value: "Riser",
            },
            Rule {
                keyword: "impact",
                category: "FX",
                value: "Impact",
            },
            Rule {
                keyword: "sweep",
                category: "FX",
                value: "Sweep",
            },
            Rule {
                keyword: "foley",
                category: "FX",
                value: "Foley",
            },
            Rule {
                keyword: "noise",
                category: "FX",
                value: "Noise",
            },
            Rule {
                keyword: "texture",
                category: "FX",
                value: "Texture",
            },
            Rule {
                keyword: "ambience",
                category: "FX",
                value: "Ambience",
            },
            Rule {
                keyword: "drone",
                category: "FX",
                value: "Drone",
            },
            Rule {
                keyword: "transition",
                category: "FX",
                value: "Transition",
            },
            // 🌍 Genres
            Rule {
                keyword: "trap",
                category: "Genre",
                value: "Trap",
            },
            Rule {
                keyword: "hiphop",
                category: "Genre",
                value: "Hip Hop",
            },
            Rule {
                keyword: "hip hop",
                category: "Genre",
                value: "Hip Hop",
            },
            Rule {
                keyword: "boom bap",
                category: "Genre",
                value: "Boom Bap",
            },
            Rule {
                keyword: "house",
                category: "Genre",
                value: "House",
            },
            Rule {
                keyword: "techno",
                category: "Genre",
                value: "Techno",
            },
            Rule {
                keyword: "drill",
                category: "Genre",
                value: "Drill",
            },
            Rule {
                keyword: "uk drill",
                category: "Genre",
                value: "UK Drill",
            },
            Rule {
                keyword: "lofi",
                category: "Genre",
                value: "Lo-Fi",
            },
            Rule {
                keyword: "lo-fi",
                category: "Genre",
                value: "Lo-Fi",
            },
            Rule {
                keyword: "cinematic",
                category: "Genre",
                value: "Cinematic",
            },
            Rule {
                keyword: "edm",
                category: "Genre",
                value: "EDM",
            },
            Rule {
                keyword: "dubstep",
                category: "Genre",
                value: "Dubstep",
            },
            Rule {
                keyword: "riddim",
                category: "Genre",
                value: "Riddim",
            },
            Rule {
                keyword: "dnb",
                category: "Genre",
                value: "Drum & Bass",
            },
            Rule {
                keyword: "drum and bass",
                category: "Genre",
                value: "Drum & Bass",
            },
            Rule {
                keyword: "amapiano",
                category: "Genre",
                value: "Amapiano",
            },
            Rule {
                keyword: "afrobeat",
                category: "Genre",
                value: "Afrobeats",
            },
            Rule {
                keyword: "afrobeats",
                category: "Genre",
                value: "Afrobeats",
            },
            Rule {
                keyword: "reggaeton",
                category: "Genre",
                value: "Reggaeton",
            },
            Rule {
                keyword: "rnb",
                category: "Genre",
                value: "R&B",
            },
            Rule {
                keyword: "r&b",
                category: "Genre",
                value: "R&B",
            },
            Rule {
                keyword: "pop",
                category: "Genre",
                value: "Pop",
            },
            Rule {
                keyword: "baile",
                category: "Genre",
                value: "Baile Funk",
            },
            Rule {
                keyword: "baile funk",
                category: "Genre",
                value: "Baile Funk",
            },
            Rule {
                keyword: "jersey",
                category: "Genre",
                value: "Jersey Club",
            },
            Rule {
                keyword: "phonk",
                category: "Genre",
                value: "Phonk",
            },
            Rule {
                keyword: "country",
                category: "Genre",
                value: "Country",
            },
            Rule {
                keyword: "rock",
                category: "Genre",
                value: "Rock",
            },
            // 🧠 Characteristics
            Rule {
                keyword: "dry",
                category: "Character",
                value: "Dry",
            },
            Rule {
                keyword: "wet",
                category: "Character",
                value: "Wet",
            },
            Rule {
                keyword: "distorted",
                category: "Character",
                value: "Distorted",
            },
            Rule {
                keyword: "clean",
                category: "Character",
                value: "Clean",
            },
            Rule {
                keyword: "vintage",
                category: "Character",
                value: "Vintage",
            },
            Rule {
                keyword: "acoustic",
                category: "Character",
                value: "Acoustic",
            },
            Rule {
                keyword: "punchy",
                category: "Character",
                value: "Punchy",
            },
            Rule {
                keyword: "soft",
                category: "Character",
                value: "Soft",
            },
            Rule {
                keyword: "hard",
                category: "Character",
                value: "Hard",
            },
            Rule {
                keyword: "dark",
                category: "Character",
                value: "Dark",
            },
            Rule {
                keyword: "bright",
                category: "Character",
                value: "Bright",
            },
            Rule {
                keyword: "warm",
                category: "Character",
                value: "Warm",
            },
            Rule {
                keyword: "cold",
                category: "Character",
                value: "Cold",
            },
            Rule {
                keyword: "analog",
                category: "Character",
                value: "Analog",
            },
            Rule {
                keyword: "digital",
                category: "Character",
                value: "Digital",
            },
            Rule {
                keyword: "gritty",
                category: "Character",
                value: "Gritty",
            },
            Rule {
                keyword: "saturated",
                category: "Character",
                value: "Saturated",
            },
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

        let is_cymbal_or_fx = path_str.contains("crash")
            || path_str.contains("ride")
            || path_str.contains("cymbal")
            || path_str.contains("fx")
            || path_str.contains("impact")
            || path_str.contains("riser")
            || path_str.contains("pad")
            || path_str.contains("808");

        let is_explicit_loop = path_str.contains("loop") || path_str.contains("break");

        let is_explicit_oneshot = path_str.contains("oneshot")
            || path_str.contains("one-shot")
            || path_str.contains("one shot");

        let format = if is_explicit_oneshot {
            "One-Shot"
        } else if is_explicit_loop {
            "Loop"
        } else if duration_ms < 3500 {
            "One-Shot"
        } else if is_cymbal_or_fx {
            "One-Shot"
        } else {
            "Loop"
        };

        found_tags.insert(serde_json::json!({ "category": "Format", "value": format }));

        // Such-Durchlauf
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

        // ==========================================
        // AUTO-INFERENCE LOGIC (Broad Classes hinzufügen)
        // ==========================================
        let broad_classes = vec![
            "Drums",
            "Percussion",
            "Bass",
            "Synth",
            "Keys",
            "Guitar",
            "Strings",
            "Brass and Woodwinds",
            "Vocals",
            "FX",
        ];
        for &class in &broad_classes {
            if categories_found.contains(class) {
                // Fügt den übergeordneten Tag (z.B. "Drums") hinzu, wenn ein Kind (z.B. "Kick") gefunden wurde
                found_tags.insert(serde_json::json!({ "category": class, "value": class }));
            }
        }

        // Bass Auto-Inference spezifisch
        if path_str.contains("808") {
            found_tags.insert(serde_json::json!({ "category": "Bass", "value": "Bass" }));
            found_tags.insert(serde_json::json!({ "category": "Bass", "value": "808" }));
        }

        let mut tags_vec: Vec<serde_json::Value> = found_tags.into_iter().collect();

        // ==========================================
        // PRIORITY SORTING (Wichtigstes nach vorne!)
        // ==========================================
        tags_vec.sort_by(|a, b| {
            let cat_a = a["category"].as_str().unwrap_or("");
            let val_a = a["value"].as_str().unwrap_or("");
            let cat_b = b["category"].as_str().unwrap_or("");
            let val_b = b["value"].as_str().unwrap_or("");

            // Gewichtungs-System (Niedriger ist besser/weiter vorne)
            fn get_prio(cat: &str, val: &str) -> i32 {
                if cat == val {
                    return 1;
                } // 1. Platz: Hauptklassen (z.B. "Drums", "Percussion")

                match cat {
                    "Drums"
                    | "Percussion"
                    | "Bass"
                    | "Synth"
                    | "Keys"
                    | "Guitar"
                    | "Strings"
                    | "Vocals"
                    | "Brass and Woodwinds"
                    | "FX" => 2, // 2. Platz: Spezifische Instrumente (z.B. "Kick", "Snare")
                    "Format" => 3,    // 3. Platz: "One-Shot" oder "Loop"
                    "Genre" => 4,     // 4. Platz: "Trap", "Baile Funk"
                    "Character" => 5, // 5. Platz: "Punchy", "Vintage"
                    _ => 6,
                }
            }

            let prio_a = get_prio(cat_a, val_a);
            let prio_b = get_prio(cat_b, val_b);

            // Sortieren: Erst nach Gewicht, dann Alphabetisch bei Gleichstand
            prio_a
                .cmp(&prio_b)
                .then_with(|| cat_a.cmp(cat_b))
                .then_with(|| val_a.cmp(val_b))
        });

        tags_vec
    }
}
