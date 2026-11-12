//! Embedded, offline help. Guide bodies intentionally remain in English.
use crate::locale::Catalog;
use eframe::egui;
mod markdown;
mod search;
pub const TOPICS: &[(&str, &str)] = &[
    ("music", "Start making music"),
    ("mini", "Mini-notation"),
    ("sounds", "Sound library / banks"),
    ("daw", "DAW workflow"),
    ("timeline", "Musical time and transport"),
    ("recording", "Recording and calibration"),
    ("editing", "Editing and mixing"),
    ("voicebank", "Create your first voicebank"),
    ("voicebank-reference", "Voicebanks"),
    ("mixing", "Routing and native DSP"),
    ("plugins", "Plugin compatibility"),
    ("phonetics", "Phonetics and recording sounds"),
    ("phonetics-en", "English sound checklist"),
    ("phonetics-de", "German sound checklist"),
    ("phonetics-fr", "French sound checklist"),
    ("phonetics-ja", "Japanese sound checklist"),
    ("phonetics-la", "Latin sound checklist"),
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
const LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"),
    ("de", "German"),
    ("fr", "French"),
    ("ja", "Japanese"),
    ("la", "Latin (voicebanks only)"),
];
const CATEGORIES: &[(&str, &[&str])] = &[
    ("Getting Started", &["music", "daw"]),
    ("Composition", &["mini", "sounds", "timeline"]),
    ("Recording & Mixing", &["recording", "editing", "mixing"]),
    (
        "Voicebanks",
        &[
            "voicebank",
            "voicebank-reference",
            "phonetics",
            "phonetics-en",
            "phonetics-de",
            "phonetics-fr",
            "phonetics-ja",
            "phonetics-la",
        ],
    ),
    ("Plugins", &["plugins"]),
    (
        "Reference",
        &[
            "commands",
            "operations",
            "platforms",
            "language",
            "language-research",
        ],
    ),
];
pub fn document(topic: &str) -> Option<&'static str> {
    Some(match topic {
        "mini" => include_str!("../docs/MINI_NOTATION.md"),
        "sounds" => include_str!("../docs/SOUNDS.md"),
        "daw" => include_str!("../docs/DAW_EXPANSION.md"),
        "timeline" => include_str!("../docs/TIMELINE.md"),
        "recording" => include_str!("../docs/RECORDING.md"),
        "editing" => include_str!("../docs/EDITING.md"),
        "music" => include_str!("../docs/MAKING_MUSIC.md"),
        "voicebank" => include_str!("../docs/FIRST_VOICEBANK.md"),
        "voicebank-reference" => include_str!("../docs/VOICEBANKS.md"),
        "mixing" => include_str!("../docs/MIXING.md"),
        "plugins" => include_str!("../docs/PLUGINS.md"),
        "phonetics" => include_str!("../docs/PHONETICS.md"),
        "phonetics-en" => include_str!("../docs/phonetics/en.md"),
        "phonetics-de" => include_str!("../docs/phonetics/de.md"),
        "phonetics-fr" => include_str!("../docs/phonetics/fr.md"),
        "phonetics-ja" => include_str!("../docs/phonetics/ja.md"),
        "phonetics-la" => include_str!("../docs/phonetics/la.md"),
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
    search_state: search::State,
    language_guide: String,
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
            search_state: search::State::default(),
            language_guide: "en".into(),
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
            self.search_state.dismiss();
            return;
        }
        let mut navigate = None;
        egui::Window::new(catalog.text("Documentation"))
            .id(egui::Id::new("documentation"))
            .open(&mut self.open)
            .default_size([780., 600.])
            .resizable(true)
            .show(ctx, |ui| {
                if let Some(topic) =
                    search::show(ui, catalog, &mut self.search, &mut self.search_state)
                {
                    self.topic = topic;
                }
                ui.add_space(6.);
                ui.horizontal_wrapped(|ui| {
                    ui.label(catalog.text("Guides are written in English."));
                    ui.separator();
                    ui.label(catalog.text("Language guide"));
                    if LANGUAGES.iter().any(|(id, _)| *id == self.topic) {
                        self.language_guide.clone_from(&self.topic);
                    }
                    let old = self.language_guide.clone();
                    let selected = LANGUAGES
                        .iter()
                        .find(|(id, _)| *id == self.language_guide)
                        .map_or("English", |(_, name)| *name);
                    egui::ComboBox::from_id_salt("documentation-language-guide")
                        .selected_text(catalog.text(selected))
                        .show_ui(ui, |ui| {
                            for (id, name) in LANGUAGES {
                                ui.selectable_value(
                                    &mut self.language_guide,
                                    (*id).into(),
                                    catalog.text(name),
                                );
                            }
                        });
                    let open_language = ui
                        .small_button(catalog.text("Open language guide"))
                        .clicked();
                    if self.language_guide != old || open_language {
                        self.topic.clone_from(&self.language_guide);
                    }
                });
                egui::ScrollArea::vertical()
                    .id_salt("documentation-categories")
                    .max_height(200.)
                    .show(ui, |ui| {
                        for (category, topics) in CATEGORIES {
                            let active = topics.contains(&self.topic.as_str());
                            let title = egui::RichText::new(catalog.text(category));
                            let title = if active {
                                title.strong().color(ui.visuals().hyperlink_color)
                            } else {
                                title
                            };
                            egui::CollapsingHeader::new(title)
                                .id_salt(("documentation-category", category))
                                .default_open(active)
                                .open((active && self.rendered_topic != self.topic).then_some(true))
                                .show(ui, |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        for id in *topics {
                                            if let Some((_, name)) =
                                                TOPICS.iter().find(|(topic, _)| topic == id)
                                                && ui
                                                    .selectable_label(
                                                        self.topic == *id,
                                                        catalog.text(name),
                                                    )
                                                    .clicked()
                                            {
                                                self.topic = (*id).into();
                                            }
                                        }
                                    });
                                });
                        }
                    });
                ui.separator();
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
        "SOUNDS.md" => "sounds",
        "DAW_EXPANSION.md" => "daw",
        "TIMELINE.md" => "timeline",
        "RECORDING.md" => "recording",
        "EDITING.md" => "editing",
        "FIRST_VOICEBANK.md" => "voicebank",
        "VOICEBANKS.md" => "voicebank-reference",
        "MIXING.md" => "mixing",
        "PLUGINS.md" => "plugins",
        "PHONETICS.md" => "phonetics",
        "en.md" => "phonetics-en",
        "de.md" => "phonetics-de",
        "fr.md" => "phonetics-fr",
        "ja.md" => "phonetics-ja",
        "la.md" => "phonetics-la",
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
    fn categories_and_language_selector_keep_every_topic_reachable_once() {
        let ids: Vec<_> = CATEGORIES
            .iter()
            .flat_map(|(_, topics)| topics.iter().copied())
            .chain(LANGUAGES.iter().map(|(id, _)| *id))
            .collect();
        let unique: std::collections::BTreeSet<_> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len());
        assert_eq!(unique.len(), TOPICS.len());
        for (id, _) in TOPICS {
            assert!(unique.contains(id), "hidden documentation topic {id}");
        }
    }
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
    fn phonetics_checklists_follow_shipped_inventories() {
        for (tag, profile) in [
            ("en", include_str!("../resources/languages/en.toml")),
            ("de", include_str!("../resources/languages/de.toml")),
            ("fr", include_str!("../resources/languages/fr.toml")),
            ("ja", include_str!("../resources/languages/ja.toml")),
            ("la", include_str!("../resources/languages/la.toml")),
        ] {
            let profile: toml::Value = toml::from_str(profile).unwrap();
            let guide = document(&format!("phonetics-{tag}")).unwrap();
            let phones = profile["phones"].as_array().unwrap();
            for phone in phones {
                let id = phone["id"].as_str().unwrap();
                assert!(guide.contains(&format!("| `{id}` |")), "missing {tag}/{id}");
            }
            assert_eq!(
                guide
                    .lines()
                    .filter(|line| line.starts_with("| [ ] |"))
                    .count(),
                phones.len()
            );
            assert_eq!(
                topic_for_path(&format!("phonetics/{tag}.md")),
                Some(format!("phonetics-{tag}").as_str())
            );
        }
    }
    #[test]
    fn sound_catalogue_matches_native_voices() {
        let guide = document("sounds").unwrap();
        for name in crate::dsp::DRUM_NAMES {
            assert!(
                guide.contains(&format!("`{name}`")),
                "missing native sound {name}"
            );
        }
        assert_eq!(topic_for_path("SOUNDS.md"), Some("sounds"));
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
