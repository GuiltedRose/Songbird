//! Small native compatibility frontend, not a JavaScript interpreter.
use anyhow::{Context, Result, ensure};
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Control {
    pub name: String,
    pub pattern: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Playback {
    pub end: f64,
    pub decay: Option<f32>,
    pub sustain: f32,
}
impl Default for Playback {
    fn default() -> Self {
        Self {
            end: 1.,
            decay: None,
            sustain: 0.,
        }
    }
}
struct Expression<'a> {
    text: &'a str,
    at: usize,
}
impl Expression<'_> {
    fn ws(&mut self) {
        while self
            .text
            .as_bytes()
            .get(self.at)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.at += 1;
        }
    }
    fn take(&mut self, token: &str) -> Result<()> {
        self.ws();
        ensure!(
            self.text[self.at..].starts_with(token),
            "expected {token:?} in sound expression"
        );
        self.at += token.len();
        Ok(())
    }
    fn name(&mut self) -> Result<String> {
        self.ws();
        let start = self.at;
        while self
            .text
            .as_bytes()
            .get(self.at)
            .is_some_and(|c| c.is_ascii_alphabetic())
        {
            self.at += 1;
        }
        ensure!(self.at > start, "expected a function name");
        Ok(self.text[start..self.at].into())
    }
    fn argument(&mut self) -> Result<String> {
        self.take("(")?;
        self.ws();
        let quote = self
            .text
            .as_bytes()
            .get(self.at)
            .copied()
            .filter(|c| *c == b'\'' || *c == b'"');
        let value = if let Some(quote) = quote {
            self.at += 1;
            let mut value = String::new();
            let mut closed = false;
            while self.at < self.text.len() {
                let c = self.text[self.at..].chars().next().unwrap();
                self.at += c.len_utf8();
                if c as u32 == quote as u32 {
                    closed = true;
                    break;
                }
                if c == '\\' {
                    let escaped = self.text[self.at..]
                        .chars()
                        .next()
                        .context("incomplete string escape")?;
                    ensure!(
                        ['\\', '\'', '"'].contains(&escaped),
                        "only quote/backslash escapes are supported"
                    );
                    self.at += escaped.len_utf8();
                    value.push(escaped);
                } else {
                    value.push(c);
                }
            }
            ensure!(closed, "unclosed sound expression string");
            value
        } else {
            let start = self.at;
            while self
                .text
                .as_bytes()
                .get(self.at)
                .is_some_and(|c| *c != b')')
            {
                self.at += 1;
            }
            self.text[start..self.at].trim().into()
        };
        ensure!(
            !value.is_empty() && value.len() <= 8192,
            "sound argument must contain 1..8192 bytes"
        );
        self.take(")")?;
        Ok(value)
    }
}
fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
fn expression(text: &str) -> Result<String> {
    let mut parser = Expression { text, at: 0 };
    let constructor = parser.name()?;
    ensure!(
        ["s", "sound", "note"].contains(&constructor.as_str()),
        "expected s, sound, or note constructor"
    );
    let pitched = constructor == "note";
    let mut instrument = None;
    let mut pattern = parser.argument()?;
    let mut controls: Vec<Control> = Vec::new();
    let mut methods = 0;
    loop {
        parser.ws();
        let rest = parser.text[parser.at..].trim();
        if rest.is_empty() || rest == ";" || rest.starts_with('#') || rest.starts_with("//") {
            break;
        }
        parser.take(".")?;
        let method = parser.name()?;
        let value = parser.argument()?;
        methods += 1;
        ensure!(methods <= 16, "maximum 16 chained functions");
        match method.as_str() {
            "s" | "sound" if pitched => {
                let name = crate::instruments::alias(&value);
                ensure!(
                    crate::instruments::NAMES.contains(&name)
                        || [
                            "sine",
                            "saw",
                            "square",
                            "triangle",
                            "noise",
                            "analog",
                            "soft-lead",
                            "sample"
                        ]
                        .contains(&name),
                    "note().s() requires one pitched instrument name"
                );
                instrument = Some(value);
            }
            "fast" | "slow" => {
                let factor: f64 = value
                    .parse()
                    .context("fast/slow requires a positive numeric scalar")?;
                ensure!(
                    factor.is_finite() && factor > 0. && factor <= 1024.,
                    "fast/slow factor range (0,1024]"
                );
                let operator = if method == "fast" { '*' } else { '/' };
                pattern = format!("[{pattern}]{operator}{factor}");
                for control in &mut controls {
                    control.pattern = format!("[{}]{operator}{factor}", control.pattern);
                }
            }
            "end" | "dec" | "decay" | "sustain" | "sus" => {
                let name = match method.as_str() {
                    "end" => "end",
                    "sustain" | "sus" => "sustain",
                    _ => "decay",
                };
                controls.retain(|control| control.name != name);
                controls.push(Control {
                    name: name.into(),
                    pattern: value,
                });
            }
            _ => anyhow::bail!(
                "unsupported chained function {method:?}; use native Songbird directives for other controls"
            ),
        }
    }
    crate::mini::Pattern::compile(&pattern)?;
    let mut output = if pitched {
        let selected = instrument.map_or(String::new(), |name| {
            format!("instrument {}\n", quoted(&name))
        });
        format!(
            "notation mini\n{selected}notes {}\nmini-sound\n",
            quoted(&pattern)
        )
    } else {
        format!(
            "notation mini\ninstrument drumkit\npattern {}\nmini-sound\n",
            quoted(&pattern)
        )
    };
    for control in controls {
        crate::mini::Pattern::compile(&control.pattern)?;
        output.push_str(&format!(
            "mini-control {} {}\n",
            control.name,
            quoted(&control.pattern)
        ));
    }
    Ok(output)
}
/// Translate standalone lines or expressions inside native track blocks. Native
/// strings/comments remain untouched, including multiline quoted Mini patterns.
pub fn translate(source: &str) -> Result<String> {
    let lines: Vec<_> = source.lines().collect();
    let mut output = String::with_capacity(source.len());
    let mut depth = 0i32;
    let mut track_depth = None;
    let mut quote = false;
    let mut escaped = false;
    let mut i = 0;
    let mut generated = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        let candidate = trimmed
            .strip_prefix("$:")
            .map(str::trim_start)
            .unwrap_or(trimmed);
        let function_end = candidate
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(candidate.len());
        let starts = ["s", "sound", "note"].contains(&&candidate[..function_end])
            && candidate[function_end..].trim_start().starts_with('(');
        if !quote && starts {
            let mut text = candidate.to_owned();
            while i + 1 < lines.len() && lines[i + 1].trim_start().starts_with('.') {
                i += 1;
                text.push(' ');
                text.push_str(lines[i].trim());
            }
            let translated = expression(&text)
                .with_context(|| format!("sound expression ending at line {}", i + 1))?;
            if track_depth.is_none() {
                generated += 1;
                output.push_str(&format!(
                    "track __sound_{generated} {{\nloop\n{translated}}}\n"
                ));
            } else {
                output.push_str(&translated);
            }
        } else {
            ensure!(
                quote || !trimmed.starts_with("$:"),
                "$: must prefix a supported sound expression"
            );
            output.push_str(line);
            output.push('\n');
            let opens_track = !quote
                && trimmed
                    .strip_prefix("track")
                    .is_some_and(|rest| rest.chars().next().is_some_and(char::is_whitespace));
            for c in line.chars() {
                if quote {
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == '"' {
                        quote = false;
                    }
                } else {
                    match c {
                        '#' => break,
                        '"' => quote = true,
                        '{' => {
                            depth += 1;
                            if opens_track && track_depth.is_none() {
                                track_depth = Some(depth);
                            }
                        }
                        '}' => {
                            if track_depth == Some(depth) {
                                track_depth = None;
                            }
                            depth -= 1;
                        }
                        _ => {}
                    }
                }
            }
        }
        i += 1;
    }
    Ok(output)
}
pub fn drum_alias(name: &str) -> &str {
    match name {
        "bd" => "kick",
        "sd" => "snare",
        "hh" => "hat",
        "oh" => "open-hat",
        "cp" => "clap",
        "rim" => "rimshot",
        "cr" => "crash",
        "rd" => "ride",
        "ht" => "high-tom",
        "mt" => "mid-tom",
        "lt" => "low-tom",
        "sh" => "shaker",
        "cb" => "cowbell",
        "tb" => "tambourine",
        _ => name,
    }
}
pub fn controls(
    events: &mut [crate::language::Event],
    track: &crate::language::Track,
    cycles: f64,
) -> Result<()> {
    if !track.mini_sound && track.mini_controls.is_empty() {
        return Ok(());
    }
    ensure!(
        track.mini && track.voice.is_none(),
        "sound/note constructors require a Mini instrumental track"
    );
    for event in events.iter_mut() {
        ensure!(
            if track.instrument == "drumkit" {
                event.drum.is_some()
            } else {
                event.drum.is_none() && !event.notes.is_empty()
            },
            "sound patterns require percussion labels on drumkit tracks and pitches on instrumental tracks"
        );
        event.playback = Some(Playback {
            end: 1.,
            decay: None,
            sustain: 0.,
        });
    }
    for control in &track.mini_controls {
        let values =
            crate::mini::Pattern::compile(&control.pattern)?.control_values(cycles, track.seed)?;
        ensure!(
            values.windows(2).all(|p| p[0].1 <= p[1].0 + 1e-10),
            "overlapping sampler control values are unsupported"
        );
        let values = values
            .into_iter()
            .map(|(start, end, text)| {
                let value: f64 = text
                    .parse()
                    .context("sampler control patterns must contain numbers")?;
                ensure!(
                    value.is_finite()
                        && match control.name.as_str() {
                            "end" | "sustain" => (0. ..=1.).contains(&value),
                            "decay" => (0. ..=10.).contains(&value),
                            _ => false,
                        },
                    "end/sustain range 0..1; decay range 0..10 seconds"
                );
                Ok((start, end, value))
            })
            .collect::<Result<Vec<_>>>()?;
        for event in events.iter_mut() {
            let cycle = event.beat / track.cycle_beats;
            let index = values.partition_point(|(start, _, _)| *start <= cycle + 1e-10);
            let value = index
                .checked_sub(1)
                .and_then(|i| values.get(i))
                .filter(|(_, end, _)| cycle < *end - 1e-10)
                .map(|(_, _, value)| value)
                .context("sampler control must supply one numeric value at every sound onset")?;
            let playback = event.playback.as_mut().unwrap();
            match control.name.as_str() {
                "end" => playback.end = *value,
                "sustain" => playback.sustain = *value as f32,
                _ => playback.decay = Some(*value as f32),
            }
        }
    }
    Ok(())
}
