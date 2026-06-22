//! `cmd_duration` segment — reads the elapsed ms from the
//! `SEKI_CMD_DURATION_MS` env var (populated by the shell-init hook)
//! and renders if it meets `min_time`. Falls back to silent when
//! the env var is unset.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::cmd_duration::CmdDurationConfig,
    segment::StyledFragment,
};

pub struct CmdDurationModule {
    cfg: CmdDurationConfig,
}

impl CmdDurationModule {
    pub fn new(cfg: CmdDurationConfig) -> Self {
        Self { cfg }
    }
}

impl Module for CmdDurationModule {
    fn name(&self) -> &'static str {
        "cmd_duration"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, _ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let Some(ms) = read_duration_ms() else {
            return Ok(None);
        };
        if ms < self.cfg.min_time {
            return Ok(None);
        }
        let duration = format_duration(ms, self.cfg.show_milliseconds);
        let text = seki_core::format::render_one(&self.cfg.format, "duration", &duration);
        Ok(Some(Segment::new("cmd_duration").push(StyledFragment::new(
            text,
            self.cfg.style.resolve(),
        ))))
    }
}

fn read_duration_ms() -> Option<u64> {
    std::env::var("SEKI_CMD_DURATION_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
}

pub fn format_duration(ms: u64, show_ms: bool) -> String {
    let secs = ms / 1000;
    let rem_ms = ms % 1000;
    let mins = secs / 60;
    let rem_secs = secs % 60;
    let hours = mins / 60;
    let rem_mins = mins % 60;

    let mut s = String::new();
    if hours > 0 {
        s.push_str(&hours.to_string());
        s.push('h');
    }
    if rem_mins > 0 || hours > 0 {
        s.push_str(&rem_mins.to_string());
        s.push('m');
    }
    s.push_str(&rem_secs.to_string());
    if show_ms && rem_ms > 0 {
        s.push_str(&format_3digit(rem_ms));
        s.push_str("ms");
    } else {
        s.push('s');
    }
    s
}

fn format_3digit(ms: u64) -> String {
    // pad-left so 5 ms reads `005ms`
    let raw = ms.to_string();
    if raw.len() >= 3 {
        raw
    } else {
        let mut s = String::with_capacity(3);
        for _ in 0..(3 - raw.len()) {
            s.push('0');
        }
        s.push_str(&raw);
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seki_core::format::render_one;

    #[test]
    fn format_seconds() {
        assert_eq!(format_duration(5_000, false), "5s");
    }

    #[test]
    fn format_minutes_and_seconds() {
        assert_eq!(format_duration(125_000, false), "2m5s");
    }

    #[test]
    fn format_hours_minutes_seconds() {
        assert_eq!(format_duration(3_725_000, false), "1h2m5s");
    }

    #[test]
    fn format_with_milliseconds() {
        assert_eq!(format_duration(2_345, true), "2345ms");
    }

    #[test]
    fn render_strips_starship_markup() {
        assert_eq!(
            render_one("[$duration]($style) ", "duration", "5s"),
            "5s "
        );
    }
}
