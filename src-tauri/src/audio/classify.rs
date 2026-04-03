pub fn classify_by_filename(filename: &str) -> Option<String> {
    let name = filename.to_lowercase();

    // Die Reihenfolge ist wichtig: Spezifische Begriffe vor generischen
    if name.contains("kick") || name.contains("bd") {
        return Some("Kick".to_string());
    }
    if name.contains("snare") || name.contains("sd") || name.contains("rim") {
        return Some("Snare".to_string());
    }
    if name.contains("clap") {
        return Some("Clap".to_string());
    }
    if name.contains("hat") || name.contains("hh") || name.contains("hihat") {
        return Some("HiHat".to_string());
    }
    if name.contains("cymbal") || name.contains("crash") || name.contains("ride") {
        return Some("Cymbal".to_string());
    }
    if name.contains("perc")
        || name.contains("tom")
        || name.contains("bongo")
        || name.contains("shaker")
    {
        return Some("Percussion".to_string());
    }
    if name.contains("bass")
        || name.contains("808")
        || name.contains("sub")
        || name.contains("reese")
    {
        return Some("Bass".to_string());
    }
    if name.contains("vocal")
        || name.contains("vox")
        || name.contains("acapella")
        || name.contains("chant")
    {
        return Some("Vocal".to_string());
    }
    if name.contains("fx")
        || name.contains("riser")
        || name.contains("impact")
        || name.contains("sweep")
        || name.contains("noise")
        || name.contains("sfx")
    {
        return Some("FX".to_string());
    }
    if name.contains("synth")
        || name.contains("pad")
        || name.contains("lead")
        || name.contains("chord")
        || name.contains("arp")
        || name.contains("pluck")
    {
        return Some("Synth".to_string());
    }
    if name.contains("loop") || name.contains("bpm") {
        return Some("Loop".to_string());
    }

    None
}
