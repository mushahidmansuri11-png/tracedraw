//! File > Pack Studio: the flat design side of a pack. Three tabs: Box
//! (the dieline, its margins, the panel fills and the guides), Nutrition
//! (a table for a country) and Export for print (Export as PDF set to a
//! PDF/X level with the carton's bleed and printers' marks). A preview
//! beside the form shows the dieline or the table as it will be drawn.
//! See docs/behavior/pack-studio.md.

use crate::app::App;
use crate::i18n::tr;
use crate::pack_studio::{
    nutrition_items, BoxStyle, Country, ExportContent, Item, PackStudioState, Tab, MM_PT,
};
use crate::ui::dialogs::{window, PDF_STANDARDS};
use crate::ui::tips::RichTip;
use egui::{Align2, Color32, Context, FontId, Stroke, Ui, Vec2};
use tracedraw_core::geometry::{Point, Rect};
use tracedraw_core::TextAlign;

/// The form's width and the preview's size, in points.
const FORM_W: f32 = 340.0;
const PREVIEW: Vec2 = Vec2::new(300.0, 300.0);

fn tip(key: &str) -> String {
    crate::ui::tips::keyed(key)
}

/// A label and a field in a grid row.
fn row(ui: &mut Ui, key: &str, add: impl FnOnce(&mut Ui)) {
    ui.label(tr(key));
    add(ui);
    ui.end_row();
}

/// A millimetre field.
fn mm(ui: &mut Ui, v: &mut f64, lo: f64, hi: f64, key: &str) {
    ui.add(
        crate::ui::field::NumField::new(v)
            .range(lo..=hi)
            .speed(0.5)
            .max_decimals(2)
            .suffix(" mm"),
    )
    .rich_tip(tip(key));
}

/// A field in the units its row names (grams, milligrams and so on).
fn amount(ui: &mut Ui, v: &mut f64, key: &str) {
    ui.add(
        crate::ui::field::NumField::new(v)
            .range(0.0..=100_000.0)
            .speed(0.1)
            .max_decimals(2),
    )
    .rich_tip(tip(key));
}

fn check(ui: &mut Ui, v: &mut bool, key: &str) {
    ui.checkbox(v, tr(key)).rich_tip(tip(key));
}

fn text(ui: &mut Ui, v: &mut String, key: &str) {
    ui.add(egui::TextEdit::singleline(v).desired_width(180.0))
        .rich_tip(tip(key));
}

fn colour(ui: &mut Ui, c: &mut [u8; 3], key: &str) {
    ui.color_edit_button_srgb(c).rich_tip(tip(key));
}

fn country_box(ui: &mut Ui, country: &mut Country, id: &str) {
    crate::ui::keynav::ComboBox::from_id_salt(id)
        .selected_text(tr(country.key()))
        .width(200.0)
        .show_ui(ui, |ui| {
            for c in Country::ALL {
                ui.selectable_value(country, c, tr(c.key()));
            }
        })
        .response
        .rich_tip(tip("pack.country"));
}

// ----- previews --------------------------------------------------------

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Items drawn small in `bounds` (sheet space), layer by layer.
fn paint(ui: &mut Ui, bounds: Rect, layers: &[&[Item]], paper: Color32) {
    let (r, _) = ui.allocate_exact_size(PREVIEW, egui::Sense::hover());
    let painter = ui.painter_at(r);
    painter.rect_filled(r, 0.0, paper);
    painter.rect_stroke(
        r,
        0.0,
        Stroke::new(1.0, crate::theme::Tokens::border()),
        egui::StrokeKind::Inside,
    );
    let (w, h) = (bounds.width().max(1.0), bounds.height().max(1.0));
    let room = 10.0;
    let scale = ((r.width() as f64 - 2.0 * room) / w).min((r.height() as f64 - 2.0 * room) / h);
    if !scale.is_finite() || scale <= 0.0 {
        return;
    }
    let ox = r.left() as f64 + (r.width() as f64 - w * scale) / 2.0 - bounds.x0 * scale;
    let oy = r.top() as f64 + (r.height() as f64 - h * scale) / 2.0 - bounds.y0 * scale;
    let to = |p: Point| egui::pos2((ox + p.x * scale) as f32, (oy + p.y * scale) as f32);
    for items in layers {
        for item in items.iter() {
            match item {
                Item::Poly {
                    pts,
                    closed,
                    fill,
                    line,
                    ..
                } => {
                    let pos: Vec<egui::Pos2> = pts.iter().map(|p| to(*p)).collect();
                    if pos.len() < 2 {
                        continue;
                    }
                    let stroke = match line {
                        Some((c, wd)) => Stroke::new(((wd * scale) as f32).clamp(0.5, 3.0), rgb(*c)),
                        None => Stroke::NONE,
                    };
                    match fill {
                        Some(c) if pos.len() > 2 => {
                            painter.add(egui::Shape::convex_polygon(pos, rgb(*c), stroke));
                        }
                        _ if *closed => {
                            painter.add(egui::Shape::closed_line(pos, stroke));
                        }
                        _ => {
                            painter.add(egui::Shape::line(pos, stroke));
                        }
                    }
                }
                Item::Text {
                    at,
                    spans,
                    size_pt,
                    align,
                    color,
                } => {
                    let px = (size_pt * MM_PT * scale) as f32;
                    if px < 3.0 {
                        continue;
                    }
                    let anchor = match align {
                        TextAlign::Center => Align2::CENTER_BOTTOM,
                        TextAlign::Right => Align2::RIGHT_BOTTOM,
                        _ => Align2::LEFT_BOTTOM,
                    };
                    let all: String = spans.iter().map(|s| s.0.as_str()).collect();
                    painter.text(to(*at), anchor, all, FontId::proportional(px), rgb(*color));
                }
            }
        }
    }
}

// ----- the tabs --------------------------------------------------------

fn box_tab(ui: &mut Ui, st: &mut PackStudioState) {
    egui::Grid::new("pack_box")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            row(ui, "pack.style", |ui| {
                crate::ui::keynav::ComboBox::from_id_salt("pack_style")
                    .selected_text(tr(st.spec.style.key()))
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        for s in BoxStyle::ALL {
                            ui.selectable_value(&mut st.spec.style, s, tr(s.key()));
                        }
                    })
                    .response
                    .rich_tip(tip("pack.style"));
            });
            let style = st.spec.style;
            row(ui, "pack.width", |ui| {
                mm(ui, &mut st.spec.width, 5.0, 2000.0, "pack.width")
            });
            row(ui, "pack.height", |ui| {
                mm(ui, &mut st.spec.height, 5.0, 2000.0, "pack.height")
            });
            if style != BoxStyle::Pouch {
                row(ui, "pack.depth", |ui| {
                    mm(ui, &mut st.spec.depth, 5.0, 2000.0, "pack.depth")
                });
            }
            let glue = if style == BoxStyle::Pouch {
                "pack.fin"
            } else {
                "pack.glue"
            };
            row(ui, glue, |ui| mm(ui, &mut st.spec.glue, 3.0, 200.0, glue));
            if style != BoxStyle::Sleeve {
                let tuck = if style == BoxStyle::Pouch {
                    "pack.crimp"
                } else {
                    "pack.tuck"
                };
                row(ui, tuck, |ui| mm(ui, &mut st.spec.tuck, 3.0, 200.0, tuck));
            }
            row(ui, "pack.bleed", |ui| {
                mm(ui, &mut st.spec.bleed, 0.0, 50.0, "pack.bleed")
            });
            row(ui, "pack.safe", |ui| {
                mm(ui, &mut st.spec.safe, 0.0, 100.0, "pack.safe")
            });
            row(ui, "pack.country", |ui| {
                country_box(ui, &mut st.nutrition.country, "pack_box_country")
            });
        });
    ui.add_space(4.0);
    check(ui, &mut st.opts.fill_panels, "pack.fill_panels");
    if st.opts.fill_panels {
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            ui.label(tr("pack.color_main"));
            colour(ui, &mut st.opts.main_color, "pack.color_main");
            ui.label(tr("pack.color_minor"));
            colour(ui, &mut st.opts.minor_color, "pack.color_minor");
        });
    }
    check(ui, &mut st.opts.starter_text, "pack.starter_text");
    if st.opts.starter_text {
        egui::Grid::new("pack_text")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                row(ui, "pack.product", |ui| text(ui, &mut st.opts.product, "pack.product"));
                row(ui, "pack.tagline", |ui| text(ui, &mut st.opts.tagline, "pack.tagline"));
                row(ui, "pack.net_weight", |ui| {
                    text(ui, &mut st.opts.net_weight, "pack.net_weight")
                });
            });
    }
    ui.add_space(4.0);
    ui.label(egui::RichText::new(tr("pack.guides")).strong());
    ui.horizontal_wrapped(|ui| {
        check(ui, &mut st.opts.show_bleed, "pack.show_bleed");
        check(ui, &mut st.opts.show_trim, "pack.show_trim");
        check(ui, &mut st.opts.show_safe, "pack.show_safe");
        check(ui, &mut st.opts.show_names, "pack.show_names");
        check(ui, &mut st.opts.show_fop, "pack.show_fop");
    });
}

fn nutrition_tab(ui: &mut Ui, st: &mut PackStudioState) {
    let n = &mut st.nutrition;
    egui::Grid::new("pack_nutrition")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            row(ui, "pack.country", |ui| {
                country_box(ui, &mut n.country, "pack_nut_country")
            });
            row(ui, "pack.n_serving", |ui| amount(ui, &mut n.nut.serving_g, "pack.n_serving"));
            row(ui, "pack.n_servings", |ui| amount(ui, &mut n.nut.servings, "pack.n_servings"));
            row(ui, "pack.n_energy", |ui| {
                amount(ui, &mut n.nut.energy_kcal, "pack.n_energy")
            });
            row(ui, "pack.n_fat", |ui| amount(ui, &mut n.nut.fat, "pack.n_fat"));
            row(ui, "pack.n_sat", |ui| amount(ui, &mut n.nut.sat_fat, "pack.n_sat"));
            row(ui, "pack.n_trans", |ui| amount(ui, &mut n.nut.trans_fat, "pack.n_trans"));
            row(ui, "pack.n_chol", |ui| {
                amount(ui, &mut n.nut.cholesterol_mg, "pack.n_chol")
            });
            row(ui, "pack.n_sodium", |ui| amount(ui, &mut n.nut.sodium_mg, "pack.n_sodium"));
            row(ui, "pack.n_carbs", |ui| amount(ui, &mut n.nut.carbs, "pack.n_carbs"));
            row(ui, "pack.n_fibre", |ui| amount(ui, &mut n.nut.fibre, "pack.n_fibre"));
            row(ui, "pack.n_sugars", |ui| amount(ui, &mut n.nut.sugars, "pack.n_sugars"));
            row(ui, "pack.n_added", |ui| {
                amount(ui, &mut n.nut.added_sugars, "pack.n_added")
            });
            row(ui, "pack.n_protein", |ui| amount(ui, &mut n.nut.protein, "pack.n_protein"));
            row(ui, "pack.n_vitd", |ui| amount(ui, &mut n.nut.vit_d_mcg, "pack.n_vitd"));
            row(ui, "pack.n_calcium", |ui| amount(ui, &mut n.nut.calcium_mg, "pack.n_calcium"));
            row(ui, "pack.n_iron", |ui| amount(ui, &mut n.nut.iron_mg, "pack.n_iron"));
            row(ui, "pack.n_potassium", |ui| {
                amount(ui, &mut n.nut.potassium_mg, "pack.n_potassium")
            });
            row(ui, "pack.n_width", |ui| mm(ui, &mut n.width_mm, 20.0, 400.0, "pack.n_width"));
            row(ui, "pack.n_font", |ui| {
                ui.add(
                    crate::ui::field::NumField::new(&mut n.font_pt)
                        .range(3.0..=40.0)
                        .speed(0.1)
                        .max_decimals(1)
                        .suffix(" pt"),
                )
                .rich_tip(tip("pack.n_font"));
            });
        });
    if !matches!(n.country, Country::Usa | Country::Canada) {
        check(ui, &mut n.both_columns, "pack.n_both");
    }
    check(ui, &mut st.on_back_panel, "pack.on_back");
    ui.add_space(4.0);
    ui.label(egui::RichText::new(tr(st.nutrition.country.note_key())).small());
}

fn print_tab(ui: &mut Ui, st: &mut PackStudioState) {
    let p = &mut st.print;
    egui::Grid::new("pack_print")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            row(ui, "pack.content", |ui| {
                crate::ui::keynav::ComboBox::from_id_salt("pack_content")
                    .selected_text(tr(p.content.key()))
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        for c in ExportContent::ALL {
                            ui.selectable_value(&mut p.content, c, tr(c.key()));
                        }
                    })
                    .response
                    .rich_tip(tip("pack.content"));
            });
            row(ui, "pack.standard", |ui| {
                crate::ui::keynav::ComboBox::from_id_salt("pack_standard")
                    .selected_text(PDF_STANDARDS[p.standard.clamp(1, 3)])
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        for i in 1..PDF_STANDARDS.len() {
                            ui.selectable_value(&mut p.standard, i, PDF_STANDARDS[i]);
                        }
                    })
                    .response
                    .rich_tip(tip("pack.standard"));
            });
            row(ui, "pack.bleed", |ui| {
                mm(ui, &mut st.spec.bleed, 0.0, 50.0, "pack.bleed")
            });
        });
    check(ui, &mut p.crop, "pack.mark_crop");
    check(ui, &mut p.bleed_marks, "pack.mark_bleed");
    check(ui, &mut p.registration, "pack.mark_registration");
    check(ui, &mut p.color_bars, "pack.mark_bars");
    check(ui, &mut p.file_info, "pack.mark_info");
    ui.add_space(6.0);
    ui.label(egui::RichText::new(tr("pack.print_note")).small());
}

pub fn dialog(app: &mut App, ctx: &Context, st: &mut PackStudioState, close: &mut bool) {
    let mut make_box = false;
    let mut insert = false;
    let mut export = false;
    let mut help = false;
    window(ctx, tr("pack.title")).show(ctx, |ui| {
        ui.horizontal(|ui| {
            for (t, key) in [
                (Tab::Box, "pack.tab_box"),
                (Tab::Nutrition, "pack.tab_nutrition"),
                (Tab::Print, "pack.tab_print"),
            ] {
                ui.selectable_value(&mut st.tab, t, tr(key));
            }
        });
        ui.separator();
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(FORM_W);
                egui::ScrollArea::vertical()
                    .max_height(420.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| match st.tab {
                        Tab::Box => box_tab(ui, st),
                        Tab::Nutrition => nutrition_tab(ui, st),
                        Tab::Print => print_tab(ui, st),
                    });
            });
            ui.add_space(12.0);
            ui.vertical(|ui| match st.tab {
                Tab::Nutrition => {
                    let (items, w, h) = nutrition_items(&st.nutrition);
                    paint(
                        ui,
                        Rect::new(0.0, 0.0, w, h),
                        &[items.as_slice()],
                        Color32::from_gray(0xe8),
                    );
                }
                _ => {
                    let items = st.spec.items(&st.opts, st.nutrition.country);
                    let b = st.spec.sane().bleed;
                    let (tw, th) = st.spec.trim_size();
                    paint(
                        ui,
                        Rect::new(-b - 2.0, -b - 2.0, tw + b + 2.0, th + b + 2.0),
                        &[
                            items.artwork.as_slice(),
                            items.dieline.as_slice(),
                            items.guides.as_slice(),
                        ],
                        Color32::WHITE,
                    );
                    let (tw, th) = st.spec.trim_size();
                    ui.label(format!("{} x {} mm", crate::pack_studio::num(tw), crate::pack_studio::num(th)));
                }
            });
        });
        ui.add_space(6.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let button = |ui: &mut Ui, label: String, tip_key: &str| {
                    ui.add(egui::Button::new(label).min_size(egui::vec2(90.0, 27.0)))
                        .rich_tip_always(crate::ui::tips::keyed(tip_key))
                        .clicked()
                };
                let (action, action_tip) = match st.tab {
                    Tab::Box => ("pack.make_box", "pack.make_box"),
                    Tab::Nutrition => ("pack.insert_table", "pack.insert_table"),
                    Tab::Print => ("pack.export", "pack.export"),
                };
                if button(ui, tr(action), action_tip) {
                    match st.tab {
                        Tab::Box => make_box = true,
                        Tab::Nutrition => insert = true,
                        Tab::Print => export = true,
                    }
                }
                if button(ui, tr("dialog.close"), "dialog.close") {
                    *close = true;
                }
                if button(ui, tr("dialog.help"), "dialog.help_button") {
                    help = true;
                }
            });
        });
    });
    if help {
        app.open_dialog_help();
    }
    if make_box {
        app.apply_pack_box(&st.spec, &st.opts, st.nutrition.country);
    }
    if insert {
        let target = st.table_target();
        app.insert_nutrition(&st.nutrition, target);
    }
    if export {
        // The Export as PDF dialog takes this one's place.
        app.export_for_print(st.spec.bleed, &st.print);
        *close = true;
    }
}
