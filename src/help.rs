//! Embedded, offline help. Guide bodies intentionally remain in English.
use crate::locale::Catalog;
use eframe::egui;
mod markdown;
pub const TOPICS: &[(&str, &str)] = &[
    ("music", "Start making music"),
    ("mini", "Mini-notation"),
    ("daw", "DAW workflow"),
    ("timeline", "Musical time and transport"),
    ("recording", "Recording and calibration"),
    ("editing", "Editing and mixing"),
    ("voicebank", "Create your first voicebank"),
    ("voicebank-reference", "Voicebanks"),
    ("commands", "Command reference"),
    ("operations", "Operation and recovery"),
    ("platforms", "Platform support"),
    ("language-research", "Language design"),
    ("language", "Language settings"),
    ("en", "English"),
    ("de", "German"),
    ("fr", "French"),
    ("ja", "Japanese"),
    ("la", "Latin (voicebanks only)"),
];
pub fn document(topic: &str) -> Option<&'static str> {
    Some(match topic {
        "mini" => include_str!("../docs/MINI_NOTATION.md"),
        "daw" => include_str!("../docs/DAW_EXPANSION.md"),
        "timeline" => include_str!("../docs/TIMELINE.md"),
        "recording" => include_str!("../docs/RECORDING.md"),
        "editing" => include_str!("../docs/EDITING.md"),
        "music" => include_str!("../docs/MAKING_MUSIC.md"),
        "voicebank" => include_str!("../docs/FIRST_VOICEBANK.md"),
        "voicebank-reference" => include_str!("../docs/VOICEBANKS.md"),
        "commands" => include_str!("../docs/LANGUAGE.md"),
        "operations" => include_str!("../docs/OPERATIONS.md"),
        "platforms" => include_str!("../docs/PLATFORMS.md"),
        "language-research" => include_str!("../docs/LANGUAGE_RESEARCH.md"),
        "language" | "languages" => include_str!("../docs/LANGUAGES.md"),
        "en" | "english" => include_str!("../docs/languages/english.md"),
        "de" | "german" => include_str!("../docs/languages/german.md"),
        "fr" | "french" => include_str!("../docs/languages/french.md"),
        "ja" | "japanese" => include_str!("../docs/languages/japanese.md"),
        "la" | "latin" => include_str!("../docs/languages/latin.md"),
        _ => return None,
    })
}
pub struct Help {
    pub open: bool,
    pub topic: String,
    pub search: String,
    rendered_topic: String,
    blocks: Vec<markdown::Block>,
    anchor: Option<String>,
    link_notice: Option<String>,
}
impl Default for Help {
    fn default() -> Self {
        Self {
            open: false,
            topic: "music".into(),
            search: String::new(),
            rendered_topic: String::new(),
            blocks: Vec::new(),
            anchor: None,
            link_notice: None,
        }
    }
}
impl Help {
    pub fn show(&mut self, ctx: &egui::Context, catalog: &Catalog) {
        if !self.open {
            return;
        }
        let mut navigate = None;
        egui::Window::new(catalog.text("Documentation"))
            .id(egui::Id::new("documentation"))
            .open(&mut self.open)
            .default_size([780., 600.])
            .resizable(true)
            .show(ctx, |ui| {
                ui.label(catalog.text("Guides are written in English."));
                ui.horizontal_wrapped(|ui| {
                    for (id, name) in TOPICS {
                        if ui
                            .selectable_label(self.topic == *id, catalog.text(name))
                            .clicked()
                        {
                            self.topic = (*id).into();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(catalog.text("Search documentation"));
                    ui.text_edit_singleline(&mut self.search);
                });
                ui.separator();
                if !self.search.is_empty() {
                    let query = self.search.to_lowercase();
                    let mut found = false;
                    for (id, name) in TOPICS {
                        if document(id).is_some_and(|s| s.to_lowercase().contains(&query)) {
                            found = true;
                            if ui.button(catalog.text(name)).clicked() {
                                self.topic = (*id).into();
                            }
                        }
                    }
                    if !found {
                        ui.label(catalog.text("No results"));
                    }
                }
                if self.rendered_topic != self.topic {
                    self.blocks = markdown::parse(
                        document(&self.topic).unwrap_or("Unknown help topic. Use help languages."),
                    );
                    self.rendered_topic = self.topic.clone();
                    self.link_notice = None;
                }
                if let Some(notice) = &self.link_notice {
                    ui.label(notice);
                }
                egui::ScrollArea::vertical()
                    .id_salt(("help-body", &self.topic))
                    .show(ui, |ui| {
                        navigate = markdown::show(ui, &self.blocks, self.anchor.take().as_deref());
                    });
            });
        if let Some(target) = navigate {
            if target.starts_with("https://") || target.starts_with("http://") {
                ctx.open_url(egui::OpenUrl::new_tab(target));
            } else {
                let (path, anchor) = target.split_once('#').unwrap_or((&target, ""));
                if path.is_empty() || topic_for_path(path).is_some() {
                    if let Some(topic) = topic_for_path(path) {
                        self.topic = topic.into();
                    }
                    self.anchor = (!anchor.is_empty()).then(|| anchor.into());
                    self.link_notice = None;
                    ctx.request_repaint();
                } else {
                    self.link_notice = Some(format!("This reference is not embedded: {target}"));
                }
            }
        }
    }
}
fn topic_for_path(path: &str) -> Option<&'static str> {
    Some(match path.rsplit('/').next()? {
        "MAKING_MUSIC.md" => "music",
        "MINI_NOTATION.md" => "mini",
        "DAW_EXPANSION.md" => "daw",
        "TIMELINE.md" => "timeline",
        "RECORDING.md" => "recording",
        "EDITING.md" => "editing",
        "FIRST_VOICEBANK.md" => "voicebank",
        "VOICEBANKS.md" => "voicebank-reference",
        "LANGUAGE.md" => "commands",
        "LANGUAGES.md" => "language",
        "OPERATIONS.md" => "operations",
        "PLATFORMS.md" => "platforms",
        "LANGUAGE_RESEARCH.md" => "language-research",
        "english.md" => "en",
        "german.md" => "de",
        "french.md" => "fr",
        "japanese.md" => "ja",
        "latin.md" => "la",
        _ => return None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_guides_have_headless_layout_coverage() {
        let ctx = egui::Context::default();
        for (topic, _) in TOPICS {
            let blocks = markdown::parse(document(topic).unwrap());
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.set_max_width(640.);
                    let _ = markdown::show(ui, &blocks, None);
                });
            });
        }
    }
    #[test]
    fn all_topics_render_and_local_links_resolve() {
        for (topic, _) in TOPICS {
            assert!(!markdown::parse(document(topic).unwrap()).is_empty());
        }
        assert_eq!(
            topic_for_path("../docs/FIRST_VOICEBANK.md"),
            Some("voicebank")
        );
        assert_eq!(topic_for_path("languages/japanese.md"), Some("ja"));
        assert_eq!(topic_for_path("javascript:alert(1)"), None);
    }
}
