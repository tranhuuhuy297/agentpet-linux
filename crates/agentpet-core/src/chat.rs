//! Speech-bubble line selection: the short line a pet "says" for each mood.
//!
//! Pure logic — built-in system lines, custom-or-system resolution from
//! `Config`, and deterministic phase-driven rotation. Callers pass the pet's
//! already-advancing animation phase instead of a clock, so behaviour is
//! reproducible and unit-testable without a display server. Pixels live in the
//! GTK crate; this module only decides *what* the bubble says.

use crate::config::Config;
use crate::state::PetMood;

/// Built-in lines per mood: the default bubble content, and the fallback when a
/// custom mood has no usable lines (a visible bubble is never blank).
pub fn system_lines(mood: PetMood) -> &'static [&'static str] {
    match mood {
        PetMood::Idle => &["just chillin'", "zzz…"],
        PetMood::Working => &["on it!", "crunching…", "almost there…"],
        PetMood::Waiting => &["need you 👀", "waiting on you…"],
        PetMood::Done => &["done ✅", "all wrapped up"],
        PetMood::Celebrate => &["woohoo! 🎉", "nailed it!"],
    }
}

/// The lines to rotate through for `mood`: the user's custom lines when
/// `chat_source == "custom"` and the mood has at least one non-blank line,
/// otherwise the system set.
pub fn lines_for(cfg: &Config, mood: PetMood) -> Vec<String> {
    if cfg.chat_source == "custom" {
        if let Some(custom) = cfg.chat_custom.get(mood.raw()) {
            let lines: Vec<String> = custom
                .iter()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect();
            if !lines.is_empty() {
                return lines;
            }
        }
    }
    system_lines(mood).iter().map(|s| s.to_string()).collect()
}

/// Index of the active line at `phase`: the phase divided into
/// `phase_per_line`-sized slots, wrapping over `count`. A single line (or an
/// empty/degenerate input) always maps to index 0, so it never flickers.
pub fn pick_index(phase: f64, phase_per_line: f64, count: usize) -> usize {
    if count == 0 || phase_per_line <= 0.0 {
        return 0;
    }
    ((phase / phase_per_line).max(0.0) as usize) % count
}

/// The active line for the given phase; `None` only when `lines` is empty.
pub fn pick(lines: &[String], phase: f64, phase_per_line: f64) -> Option<&str> {
    if lines.is_empty() {
        return None;
    }
    Some(lines[pick_index(phase, phase_per_line, lines.len())].as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom_cfg(mood: PetMood, lines: &[&str]) -> Config {
        let mut cfg = Config::default();
        cfg.chat_source = "custom".to_string();
        cfg.chat_custom
            .insert(mood.raw().to_string(), lines.iter().map(|s| s.to_string()).collect());
        cfg
    }

    #[test]
    fn every_mood_has_a_system_line() {
        for mood in PetMood::ALL {
            assert!(!system_lines(mood).is_empty(), "{:?} must have lines", mood);
        }
    }

    #[test]
    fn custom_lines_override_system_when_source_is_custom() {
        let cfg = custom_cfg(PetMood::Working, &["beep boop"]);
        assert_eq!(lines_for(&cfg, PetMood::Working), vec!["beep boop"]);
    }

    #[test]
    fn system_source_ignores_custom_lines() {
        let mut cfg = custom_cfg(PetMood::Working, &["beep boop"]);
        cfg.chat_source = "system".to_string();
        let expected: Vec<String> =
            system_lines(PetMood::Working).iter().map(|s| s.to_string()).collect();
        assert_eq!(lines_for(&cfg, PetMood::Working), expected);
    }

    #[test]
    fn empty_or_blank_custom_falls_back_to_system() {
        // Mood missing from chat_custom entirely.
        let cfg = custom_cfg(PetMood::Working, &["beep boop"]);
        let expected: Vec<String> =
            system_lines(PetMood::Idle).iter().map(|s| s.to_string()).collect();
        assert_eq!(lines_for(&cfg, PetMood::Idle), expected, "missing mood falls back");
        // Mood present but only blank/whitespace lines.
        let cfg = custom_cfg(PetMood::Done, &["", "   "]);
        let expected: Vec<String> =
            system_lines(PetMood::Done).iter().map(|s| s.to_string()).collect();
        assert_eq!(lines_for(&cfg, PetMood::Done), expected, "blank lines fall back");
    }

    #[test]
    fn pick_rotates_through_lines_and_wraps() {
        let lines: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        assert_eq!(pick(&lines, 0.0, 10.0), Some("a"));
        assert_eq!(pick(&lines, 10.0, 10.0), Some("b"));
        assert_eq!(pick(&lines, 20.0, 10.0), Some("c"));
        assert_eq!(pick(&lines, 30.0, 10.0), Some("a"), "wraps back to the first line");
    }

    #[test]
    fn single_line_stays_static_across_phases() {
        let lines = vec!["only".to_string()];
        for phase in [0.0, 7.0, 123.4, 9999.0] {
            assert_eq!(pick(&lines, phase, 10.0), Some("only"));
        }
    }

    #[test]
    fn pick_handles_empty_and_degenerate_inputs() {
        assert_eq!(pick(&[], 5.0, 10.0), None);
        let lines: Vec<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        assert_eq!(pick_index(-3.0, 10.0, 2), 0, "negative phase clamps to 0");
        assert_eq!(pick(&lines, 5.0, 0.0), Some("a"), "zero slot size degrades to first line");
    }
}
