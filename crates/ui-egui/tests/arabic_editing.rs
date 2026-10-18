//! Arabic word navigation must not panic or delete unrelated cell text.

use egui::{Event, Key, Modifiers, PointerButton, Pos2, Shape, vec2};
use egui_kittest::kittest::Queryable as _;
use gridcraft_engine::{
    Session,
    core::{CellRef, Value},
};
use gridcraft_ui_egui::SheetApp;
use serde_json::json;

fn editing(text: &str, formula_bar: bool) -> egui_kittest::Harness<'static, SheetApp> {
    let mut session = Session::new();
    session.new_workbook();
    session.execute("cell.set", json!({"cell": "A1", "input": text})).unwrap();
    let mut h = egui_kittest::Harness::builder().with_size(vec2(1200.0, 800.0)).build_ui_state(
        |ui, app: &mut SheetApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        SheetApp::new(session, Default::default()),
    );
    h.run_steps(4);
    if formula_bar {
        h.get_by_label(text).click();
    } else {
        h.key_press(Key::F2);
    }
    h.run_steps(2);
    assert_eq!(h.state().editor.as_ref().map(|e| e.from_formula_bar), Some(formula_bar));
    h.key_press(Key::End);
    h.run_steps(2);
    h
}

/// The right edge of the in-cell editor's text: the lowest painted copy, below the formula bar.
fn cell_editor_text_right_edge(h: &egui_kittest::Harness<'static, SheetApp>, text: &str) -> Option<Pos2> {
    fn collect(shape: &Shape, text: &str, out: &mut Vec<Pos2>) {
        match shape {
            Shape::Text(s) if s.galley.text() == text => out.push(s.pos + vec2(s.galley.rect.max.x - 1.0, s.galley.rect.center().y)),
            Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, text, out)),
            _ => {}
        }
    }
    let mut edges = Vec::new();
    for clipped in &h.output().shapes {
        collect(&clipped.shape, text, &mut edges);
    }
    edges.into_iter().max_by(|a, b| a.y.total_cmp(&b.y))
}

#[test]
fn previous_word_in_arabic_cell_does_not_panic() {
    let mut h = editing("Aurora بب", false);
    h.key_press_modifiers(Modifiers::CTRL, Key::ArrowLeft);
    h.run_steps(2);
    h.input_mut().events.push(egui::Event::Text("x".into()));
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(2);
    let sheet = h.state().session.active().unwrap().wb.active().unwrap();
    // End is the logical end at the Arabic run's left edge. Ctrl+Left follows visual
    // word order across the Latin prefix, preserving all original characters.
    assert_eq!(sheet.value(CellRef::new(0, 0)), Value::text("xAurora بب"));
}

#[test]
fn delete_previous_arabic_word_keeps_the_latin_prefix() {
    let mut h = editing("Aurora بب", false);
    h.key_press_modifiers(Modifiers::CTRL, Key::Backspace);
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(2);
    let sheet = h.state().session.active().unwrap().wb.active().unwrap();
    assert_eq!(sheet.value(CellRef::new(0, 0)), Value::text("Aurora "));
}

#[test]
fn delete_previous_arabic_word_in_formula_bar_keeps_the_prefix() {
    let mut h = editing("Aurora بب", true);
    h.key_press_modifiers(Modifiers::CTRL, Key::Backspace);
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(2);
    let sheet = h.state().session.active().unwrap().wb.active().unwrap();
    assert_eq!(sheet.value(CellRef::new(0, 0)), Value::text("Aurora "));
}

#[test]
fn previous_visual_word_crosses_arabic_from_its_right_edge() {
    let mut h = editing("Aurora بب", false);
    let pos = cell_editor_text_right_edge(&h, "Aurora بب").unwrap();
    h.input_mut().events.extend([
        Event::PointerMoved(pos),
        Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE },
        Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE },
    ]);
    h.run_steps(2);
    let state = egui::text_edit::TextEditState::load(&h.ctx, egui::Id::new("gridcraft.cell_editor")).unwrap();
    let caret = state.cursor.char_range().map(|r| (r.primary.index.0, r.primary.prefer_next_row));
    assert_eq!(caret, Some((7, true)), "the click lands on the Arabic run's right edge");
    h.key_press_modifiers(Modifiers::CTRL, Key::ArrowLeft);
    h.run_steps(2);
    h.input_mut().events.push(egui::Event::Text("x".into()));
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(2);
    let sheet = h.state().session.active().unwrap().wb.active().unwrap();
    assert_eq!(sheet.value(CellRef::new(0, 0)), Value::text("Aurora ببx"));
}
