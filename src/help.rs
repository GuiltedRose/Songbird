//! Embedded, offline help. Guide bodies intentionally remain in English.
use crate::locale::Catalog;
use eframe::egui;
pub const TOPICS: &[(&str, &str)] = &[
    ("music", "Start making music"),
    ("voicebank", "Create your first voicebank"),
    ("voicebank-reference", "Voicebanks"),
    ("commands", "Command reference"),
    ("language", "Language settings"),
    ("en", "English"),
    ("de", "German"),
    ("fr", "French"),
    ("ja", "Japanese"),
    ("la", "Latin (voicebanks only)"),
];
pub fn document(topic: &str) -> Option<&'static str> {
    Some(match topic {
        "music" => include_str!("../docs/MAKING_MUSIC.md"),
        "voicebank" => include_str!("../docs/FIRST_VOICEBANK.md"),
        "voicebank-reference" => include_str!("../docs/VOICEBANKS.md"),
        "commands" => include_str!("../docs/LANGUAGE.md"),
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
}
impl Default for Help {
    fn default() -> Self {
        Self {
            open: false,
            topic: "music".into(),
            search: String::new(),
        }
    }
}
impl Help {
    pub fn show(&mut self, ctx: &egui::Context, catalog: &Catalog) {
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
                egui::ScrollArea::both().show(ui, |ui| {
                    let mut body =
                        document(&self.topic).unwrap_or("Unknown help topic. Use help languages.");
                    ui.add(
                        egui::TextEdit::multiline(&mut body)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .desired_rows(32),
                    );
                });
            });
    }
}
