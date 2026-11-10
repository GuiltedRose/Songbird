//! Small renderer for the Markdown used by embedded guides. No HTML or remote assets.
use eframe::egui::{self, Color32, FontId, Stroke, TextFormat, text::LayoutJob};

#[derive(Debug, PartialEq)]
pub(super) enum Block {
    Heading(usize, String),
    Paragraph(String),
    Item(usize, String, String),
    Quote(String),
    Code(String),
    Table(Vec<Vec<String>>),
    Rule,
}
fn heading(line: &str) -> Option<(usize, &str)> {
    let n = line.bytes().take_while(|&b| b == b'#').count();
    (n > 0 && n <= 6)
        .then(|| line.get(n..)?.strip_prefix(' ').map(|s| (n, s)))
        .flatten()
}
fn item(line: &str) -> Option<(usize, String, &str)> {
    let trimmed = line.trim_start();
    let indent = ((line.len() - trimmed.len()) / 2).min(8);
    for prefix in ["- ", "* ", "+ "] {
        if let Some(body) = trimmed.strip_prefix(prefix) {
            return Some((indent, "•".into(), body));
        }
    }
    let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0
        && let Some(body) = trimmed[digits..].strip_prefix(". ")
    {
        return Some((indent, format!("{}.", &trimmed[..digits]), body));
    }
    None
}
fn cells(line: &str) -> Vec<String> {
    let mut code = false;
    let mut escaped = false;
    let mut parts = Vec::new();
    let mut start = 0;
    let line = line.trim().trim_matches('|');
    for (i, c) in line.char_indices() {
        if c == '|' && !code && !escaped {
            parts.push(line[start..i].trim().into());
            start = i + 1;
        }
        if c == '`' && !escaped {
            code = !code;
        }
        escaped = c == '\\' && !escaped;
    }
    parts.push(line[start..].trim().into());
    parts
}
fn table_separator(line: &str) -> bool {
    line.contains('|')
        && cells(line).iter().all(|c| {
            let c = c.trim_matches(':');
            c.len() >= 3 && c.bytes().all(|b| b == b'-')
        })
}
fn rule(line: &str) -> bool {
    ["---", "***", "___"].contains(&line.trim())
}
pub(super) fn parse(source: &str) -> Vec<Block> {
    let lines: Vec<_> = source.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end();
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            i += 1;
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let fence = trimmed
                .chars()
                .take_while(|&c| c == trimmed.chars().next().unwrap())
                .collect::<String>();
            i += 1;
            let mut code = Vec::new();
            while i < lines.len() && !lines[i].trim().starts_with(&fence) {
                code.push(lines[i]);
                i += 1;
            }
            blocks.push(Block::Code(code.join("\n")));
            if i < lines.len() {
                i += 1;
            }
            continue;
        }
        if let Some((level, text)) = heading(trimmed) {
            blocks.push(Block::Heading(level, text.into()));
        } else if rule(trimmed) {
            blocks.push(Block::Rule);
        } else if i + 1 < lines.len() && line.contains('|') && table_separator(lines[i + 1]) {
            let mut rows = vec![cells(line)];
            i += 2;
            while i < lines.len() && lines[i].contains('|') && !lines[i].trim().is_empty() {
                rows.push(cells(lines[i]));
                i += 1;
            }
            blocks.push(Block::Table(rows));
            continue;
        } else if let Some((indent, bullet, text)) = item(line) {
            blocks.push(Block::Item(indent, bullet, text.into()));
        } else if let Some(text) = trimmed.strip_prefix("> ") {
            blocks.push(Block::Quote(text.into()));
        } else {
            let mut text = trimmed.to_string();
            while i + 1 < lines.len() {
                let next = lines[i + 1].trim();
                if next.is_empty()
                    || heading(next).is_some()
                    || item(lines[i + 1]).is_some()
                    || next.starts_with('>')
                    || next.starts_with("```")
                    || next.starts_with("~~~")
                    || rule(next)
                    || (i + 2 < lines.len() && table_separator(lines[i + 2]))
                {
                    break;
                }
                text.push(' ');
                text.push_str(next);
                i += 1;
            }
            blocks.push(Block::Paragraph(text));
        }
        i += 1;
    }
    blocks
}
#[derive(Clone)]
struct Link {
    start: usize,
    end: usize,
    target: String,
}
fn inline(
    mut text: &str,
    format: &TextFormat,
    job: &mut LayoutJob,
    links: &mut Vec<Link>,
    colors: (Color32, Color32),
    depth: usize,
) {
    if depth > 8 {
        job.append(text, 0., format.clone());
        return;
    }
    while !text.is_empty() {
        let pos = text.find(['`', '*', '[', '\\']).unwrap_or(text.len());
        job.append(&text[..pos], 0., format.clone());
        text = &text[pos..];
        if text.is_empty() {
            break;
        }
        if let Some(rest) = text.strip_prefix('\\')
            && let Some(c) = rest.chars().next()
        {
            job.append(&rest[..c.len_utf8()], 0., format.clone());
            text = &rest[c.len_utf8()..];
            continue;
        }
        if let Some(rest) = text.strip_prefix('[')
            && let Some(end) = rest.find("](")
            && let Some(close) = rest[end + 2..].find(')')
        {
            let start = job.text.chars().count();
            let mut linked = format.clone();
            linked.color = colors.0;
            linked.underline = Stroke::new(1.0_f32, colors.0);
            inline(&rest[..end], &linked, job, links, colors, depth + 1);
            links.push(Link {
                start,
                end: job.text.chars().count(),
                target: rest[end + 2..end + 2 + close].into(),
            });
            text = &rest[end + 3 + close..];
            continue;
        }
        let marker = if text.starts_with("**") {
            "**"
        } else if text.starts_with('*') {
            "*"
        } else {
            "`"
        };
        if text.starts_with(marker)
            && let Some(end) = text[marker.len()..].find(marker)
        {
            let body = &text[marker.len()..marker.len() + end];
            let mut styled = format.clone();
            match marker {
                "`" => {
                    styled.font_id.family = egui::FontFamily::Monospace;
                    styled.background = Color32::from_black_alpha(30);
                    job.append(body, 0., styled);
                }
                "**" => {
                    styled.color = colors.1;
                    inline(body, &styled, job, links, colors, depth + 1);
                }
                _ => {
                    styled.italics = true;
                    inline(body, &styled, job, links, colors, depth + 1);
                }
            }
            text = &text[marker.len() * 2 + end..];
            continue;
        }
        let n = text.chars().next().unwrap().len_utf8();
        job.append(&text[..n], 0., format.clone());
        text = &text[n..];
    }
}
fn rich(
    ui: &mut egui::Ui,
    text: &str,
    size: f32,
    strong: bool,
    navigate: &mut Option<String>,
) -> egui::Response {
    let mut job = LayoutJob::default();
    let mut links = Vec::new();
    let format = TextFormat {
        font_id: FontId::proportional(size),
        color: if strong {
            ui.visuals().strong_text_color()
        } else {
            ui.visuals().text_color()
        },
        ..Default::default()
    };
    inline(
        text,
        &format,
        &mut job,
        &mut links,
        (
            ui.visuals().hyperlink_color,
            ui.visuals().strong_text_color(),
        ),
        0,
    );
    job.wrap.max_width = ui.available_width();
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let response = ui.add(egui::Label::new(galley.clone()).selectable(true));
    let pos = response.rect.min;
    if let Some(pointer) = response.hover_pos() {
        let cursor = galley.cursor_from_pos(pointer - pos).index;
        if let Some(link) = links
            .iter()
            .find(|link| link.start <= cursor && cursor < link.end)
        {
            if response.clicked() {
                *navigate = Some(link.target.clone());
            }
            return response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(&link.target);
        }
    }
    response
}
pub(super) fn slug(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-')
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .collect()
}
pub(super) fn show(ui: &mut egui::Ui, blocks: &[Block], anchor: Option<&str>) -> Option<String> {
    let mut navigate = None;
    let size = egui::TextStyle::Body.resolve(ui.style()).size;
    for (index, block) in blocks.iter().enumerate() {
        ui.push_id(index, |ui| {
            match block {
                Block::Heading(level, text) => {
                    ui.add_space(if *level == 1 { 4. } else { 12. });
                    let response = rich(
                        ui,
                        text,
                        size * match level {
                            1 => 1.8,
                            2 => 1.45,
                            _ => 1.15,
                        },
                        true,
                        &mut navigate,
                    );
                    if anchor == Some(slug(text).as_str()) {
                        response.scroll_to_me(Some(egui::Align::TOP));
                    }
                }
                Block::Paragraph(text) => {
                    rich(ui, text, size, false, &mut navigate);
                }
                Block::Item(indent, bullet, text) => {
                    ui.horizontal_top(|ui| {
                        ui.add_space(*indent as f32 * 14.);
                        ui.label(bullet);
                        ui.vertical(|ui| {
                            rich(ui, text, size, false, &mut navigate);
                        });
                    });
                }
                Block::Quote(text) => {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        rich(ui, text, size, false, &mut navigate);
                    });
                }
                Block::Code(code) => {
                    egui::Frame::new()
                        .fill(ui.visuals().code_bg_color)
                        .inner_margin(10.0_f32)
                        .show(ui, |ui| {
                            if ui.small_button("Copy").clicked() {
                                ui.ctx().copy_text(code.clone());
                            }
                            egui::ScrollArea::horizontal()
                                .id_salt("code")
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::Label::new(egui::RichText::new(code).monospace())
                                            .selectable(true)
                                            .wrap_mode(egui::TextWrapMode::Extend),
                                    );
                                });
                        });
                }
                Block::Table(rows) => {
                    egui::ScrollArea::horizontal()
                        .id_salt("table-scroll")
                        .show(ui, |ui| {
                            egui::Grid::new("table").striped(true).show(ui, |ui| {
                                let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
                                for (row_index, row) in rows.iter().enumerate() {
                                    for column in 0..columns {
                                        ui.vertical(|ui| {
                                            ui.set_max_width(280.);
                                            rich(
                                                ui,
                                                row.get(column).map_or("", String::as_str),
                                                size,
                                                row_index == 0,
                                                &mut navigate,
                                            );
                                        });
                                    }
                                    ui.end_row();
                                }
                            });
                        });
                }
                Block::Rule => {
                    ui.separator();
                }
            }
            ui.add_space(5.);
        });
    }
    navigate
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blocks_preserve_code_and_parse_tables_lists_and_unicode() {
        let blocks = parse(
            "# Guide\n\nHello\nworld.\n\n```rust\n  let x = 1;\n```\n\n- item\n  2. nested\n\n| Name | Value |\n|---|---|\n| 日本語 | `a|b` |\n",
        );
        assert_eq!(blocks[0], Block::Heading(1, "Guide".into()));
        assert_eq!(blocks[1], Block::Paragraph("Hello world.".into()));
        assert_eq!(blocks[2], Block::Code("  let x = 1;".into()));
        assert_eq!(blocks[4], Block::Item(1, "2.".into(), "nested".into()));
        assert_eq!(
            blocks[5],
            Block::Table(vec![
                vec!["Name".into(), "Value".into()],
                vec!["日本語".into(), "`a|b`".into()]
            ])
        );
    }
    #[test]
    fn inline_styles_keep_text_and_link_character_ranges() {
        let mut job = LayoutJob::default();
        let mut links = Vec::new();
        inline(
            "**Bold** *italic* `code` [日本語](LANGUAGES.md) \\*",
            &TextFormat::default(),
            &mut job,
            &mut links,
            (Color32::BLUE, Color32::WHITE),
            0,
        );
        assert_eq!(job.text, "Bold italic code 日本語 *");
        assert_eq!(links[0].target, "LANGUAGES.md");
        assert_eq!(
            job.text
                .chars()
                .skip(links[0].start)
                .take(links[0].end - links[0].start)
                .collect::<String>(),
            "日本語"
        );
        assert_eq!(
            slug("Recording and calibration"),
            "recording-and-calibration"
        );
    }
}
