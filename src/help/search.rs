//! Search interaction for the existing embedded topic index.
use super::{TOPICS, document};
use crate::locale::Catalog;
use eframe::egui;

#[derive(Default)]
pub(super) struct State {
    query: String,
    language: String,
    results: Vec<usize>,
    snippets: Vec<String>,
    selected: usize,
    visible: bool,
}
impl State {
    pub fn dismiss(&mut self) {
        self.visible = false;
    }
    fn refresh(&mut self, text: &str, catalog: &Catalog) -> bool {
        let query = text.trim().to_lowercase();
        if query == self.query && self.language == catalog.language {
            return false;
        }
        self.query = query;
        self.language.clone_from(&catalog.language);
        self.results.clear();
        self.snippets.clear();
        self.selected = 0;
        if !self.query.is_empty() {
            let mut ranked = Vec::new();
            for (index, (id, title)) in TOPICS.iter().enumerate() {
                let title = catalog.text(title).to_lowercase();
                let rank = if title == self.query {
                    0
                } else if title.starts_with(&self.query) {
                    1
                } else if title.contains(&self.query) {
                    2
                } else if document(id).is_some_and(|body| body.to_lowercase().contains(&self.query))
                {
                    3
                } else {
                    continue;
                };
                ranked.push((rank, index));
            }
            ranked.sort_unstable();
            self.results = ranked.into_iter().map(|(_, index)| index).collect();
            self.snippets = self
                .results
                .iter()
                .map(|&index| {
                    document(TOPICS[index].0)
                        .and_then(|body| {
                            body.lines()
                                .find(|line| line.to_lowercase().contains(&self.query))
                        })
                        .map_or_else(String::new, |line| line.trim().chars().take(180).collect())
                })
                .collect();
        }
        true
    }
    fn step(&mut self, forward: bool) {
        let count = self.results.len();
        if count > 0 {
            self.selected = if forward {
                (self.selected + 1) % count
            } else {
                (self.selected + count - 1) % count
            };
        }
    }
    fn selected_topic(&self) -> Option<String> {
        self.results.get(self.selected).map(|&i| TOPICS[i].0.into())
    }
}

pub(super) fn show(
    ui: &mut egui::Ui,
    catalog: &Catalog,
    text: &mut String,
    state: &mut State,
) -> Option<String> {
    let id = egui::Id::new("documentation-search");
    let was_visible = state.visible;
    let mut icon_clicked = false;
    let shortcut = ui.input_mut(|input| {
        input.consume_key(egui::Modifiers::COMMAND, egui::Key::K)
            || input.consume_key(egui::Modifiers::CTRL, egui::Key::K)
    });
    if shortcut {
        egui::Popup::close_all(ui.ctx());
        ui.memory_mut(|memory| memory.request_focus(id));
        state.visible = !text.trim().is_empty();
    }
    let focused = ui.memory(|memory| memory.has_focus(id));
    let mut scroll_selected = state.refresh(text, catalog);
    let mut chosen = None;
    // Consume navigation before TextEdit can move its cursor or surrender focus.
    if focused || state.visible {
        if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            state.dismiss();
        } else if focused && !state.query.is_empty() {
            for (key, forward) in [(egui::Key::ArrowDown, true), (egui::Key::ArrowUp, false)] {
                if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key)) {
                    if state.visible {
                        state.step(forward);
                    }
                    state.visible = true;
                    scroll_selected = true;
                }
            }
            if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
                chosen = state.selected_topic();
                state.dismiss();
                if chosen.is_some() {
                    ui.memory_mut(|memory| memory.surrender_focus(id));
                }
            }
        }
    }
    let label = ui.label(catalog.text("Search documentation"));
    let stroke = if focused {
        egui::Stroke::new(1.5_f32, ui.visuals().hyperlink_color)
    } else {
        ui.visuals().widgets.inactive.bg_stroke
    };
    let frame = egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .stroke(stroke)
        .corner_radius(5_u8)
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let (rect, icon) =
                    ui.allocate_exact_size(egui::vec2(18., 24.), egui::Sense::click());
                let color = ui.visuals().weak_text_color();
                let center = rect.center() - egui::vec2(2., 2.);
                ui.painter()
                    .circle_stroke(center, 4.5, egui::Stroke::new(1.5_f32, color));
                ui.painter().line_segment(
                    [center + egui::vec2(3.5, 3.5), center + egui::vec2(7., 7.)],
                    egui::Stroke::new(1.5_f32, color),
                );
                if icon.clicked() {
                    icon_clicked = true;
                    ui.memory_mut(|memory| memory.request_focus(id));
                    state.visible = !state.query.is_empty();
                }
                let width = (ui.available_width() - 62.).max(24.);
                let response = ui
                    .add_sized(
                        [width, 24.],
                        egui::TextEdit::singleline(text)
                            .id(id)
                            .frame(false)
                            .char_limit(256)
                            .hint_text(catalog.text("Search guides, commands, or topics…")),
                    )
                    .labelled_by(label.id)
                    .on_hover_text(
                        "Ctrl+K / Cmd+K: search · Up/Down: select · Enter: open · Escape: dismiss",
                    );
                ui.weak(
                    if ui.input(|i| i.raw.modifiers.mac_cmd) || cfg!(target_os = "macos") {
                        "⌘K"
                    } else {
                        "Ctrl+K"
                    },
                );
                response
            })
            .inner
        });
    let response = frame.inner;
    if response.changed() {
        scroll_selected |= state.refresh(text, catalog);
        state.visible = !state.query.is_empty();
    } else if chosen.is_none()
        && (response.clicked() || response.gained_focus())
        && !state.query.is_empty()
    {
        state.visible = true;
    }
    if state.query.is_empty() {
        state.dismiss();
    }
    let mut visible = state.visible;
    egui::Popup::from_response(&frame.response)
        .id(id.with("results"))
        .open_bool(&mut visible)
        .width(frame.response.rect.width())
        .close_behavior(if response.clicked() || icon_clicked {
            egui::PopupCloseBehavior::IgnoreClicks
        } else {
            egui::PopupCloseBehavior::CloseOnClickOutside
        })
        .show(|ui| {
            if state.results.is_empty() {
                ui.label(catalog.text("No results"));
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("documentation-search-results")
                    .max_height(240.)
                    .show(ui, |ui| {
                        for (position, &index) in state.results.iter().enumerate() {
                            let (topic, title) = TOPICS[index];
                            let row = ui.add_sized(
                                [ui.available_width(), 28.],
                                egui::Button::selectable(
                                    position == state.selected,
                                    catalog.text(title),
                                )
                                .wrap(),
                            );
                            if position == state.selected && scroll_selected {
                                row.scroll_to_me(None);
                            }
                            if row.clicked() {
                                chosen = Some(topic.into());
                            }
                            if let Some(snippet) =
                                state.snippets.get(position).filter(|s| !s.is_empty())
                            {
                                row.on_hover_text(snippet);
                            }
                        }
                    });
            }
        });
    state.visible = visible && chosen.is_none();
    if chosen.is_some() {
        response.surrender_focus();
    }
    if chosen.is_some() || was_visible != state.visible {
        ui.ctx().request_repaint();
    }
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key_frame(
        ctx: &egui::Context,
        text: &mut String,
        state: &mut State,
        key: egui::Key,
        modifiers: egui::Modifiers,
    ) -> Option<String> {
        let events = [true, false]
            .into_iter()
            .map(|pressed| egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            })
            .collect();
        let mut chosen = None;
        let _ = ctx.run(
            egui::RawInput {
                events,
                modifiers,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    chosen = show(ui, &Catalog::default(), text, state);
                });
            },
        );
        chosen
    }
    #[test]
    fn shortcuts_arrows_enter_and_escape_work_through_egui() {
        let ctx = egui::Context::default();
        let mut text = "recording".to_owned();
        let mut state = State::default();
        key_frame(
            &ctx,
            &mut text,
            &mut state,
            egui::Key::K,
            egui::Modifiers::COMMAND,
        );
        assert!(ctx.memory(|m| m.has_focus(egui::Id::new("documentation-search"))));
        assert!(state.visible && state.results.len() > 1);
        key_frame(
            &ctx,
            &mut text,
            &mut state,
            egui::Key::ArrowDown,
            egui::Modifiers::NONE,
        );
        assert_eq!(state.selected, 1);
        let expected = state.selected_topic();
        assert_eq!(
            key_frame(
                &ctx,
                &mut text,
                &mut state,
                egui::Key::Enter,
                egui::Modifiers::NONE
            ),
            expected
        );
        assert!(!state.visible);
        key_frame(
            &ctx,
            &mut text,
            &mut state,
            egui::Key::K,
            egui::Modifiers::CTRL,
        );
        assert!(state.visible);
        key_frame(
            &ctx,
            &mut text,
            &mut state,
            egui::Key::Escape,
            egui::Modifiers::NONE,
        );
        assert!(!state.visible);
        assert_eq!(text, "recording");
        assert!(ctx.memory(|m| m.has_focus(egui::Id::new("documentation-search"))));
    }
    #[test]
    fn existing_body_search_and_title_ranking_are_preserved() {
        let mut state = State::default();
        let catalog = Catalog::default();
        assert!(state.refresh("  Mini-notation  ", &catalog));
        assert_eq!(state.selected_topic().as_deref(), Some("mini"));
        assert!(!state.refresh("mini-notation", &catalog));
        state.refresh("microphone", &catalog);
        assert!(
            state
                .results
                .iter()
                .any(|&index| TOPICS[index].0 == "voicebank")
        );
        state.refresh("no-such-documentation-topic-583902", &catalog);
        assert!(state.results.is_empty());
        assert!(state.selected_topic().is_none());
        state.step(true);
        state.step(false);
        state.dismiss();
        assert!(!state.visible);
    }
    #[test]
    fn keyboard_selection_wraps_and_query_changes_reset_it() {
        let mut state = State {
            results: vec![0, 1, 2],
            visible: true,
            ..Default::default()
        };
        state.step(false);
        assert_eq!(state.selected, 2);
        state.step(true);
        assert_eq!(state.selected, 0);
        state.step(true);
        assert_eq!(state.selected, 1);
        state.refresh("recording", &Catalog::default());
        assert_eq!(state.selected, 0);
        state.dismiss();
        assert!(!state.visible);
    }
}
