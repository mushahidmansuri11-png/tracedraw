# Pack Studio

File > Pack Studio opens a dialog for the flat design of a pack: the
dieline of a box or pouch with its guides, a nutrition table for a
country, and Export for print. It draws ordinary curves and text, so
everything can be edited afterwards with the usual tools.

## Box

Style: Tuck-end carton, Sleeve or Pillow pouch (flat). Sizes in
millimetres: Width, Height, Depth (not for a pouch), Glue flap (a pouch's
Fin seal) and Tuck lip (a pouch's Crimp seal; a sleeve has none), then
Bleed (3 by default) and Safe margin (5 by default). The preview beside
the form shows the artwork, the dieline and the guides, and the trim
size.

Create box:

- sets the page to the trim size and the drawing's bleed to the Bleed
  field (Document Properties shows it);
- makes three layers: Pack artwork (prints), Pack dieline (not printed)
  and Pack guides (not printed); the layers of an earlier box on the
  page go first, so Create box again redraws the box;
- all of it is two undo steps ("Pack Studio").

Trim size: a tuck-end carton is glue + 2 x width + 2 x depth wide and
tuck + 2 x depth + height high (100 x 150 x 60 with a 12 mm glue flap
and tuck lip is 332 x 282 mm); a sleeve is the same width and the height
alone; a pouch is twice its width wide (the back's left half, the front
and the back's right half side by side) and its height high.

| Layer | Holds |
|---|---|
| Pack artwork | Panel fills over the bleed on the outer edges (Main colour for the front and back, Side colour for the sides, lid, bottom and a pouch's back halves); starter text: product name, tagline and net weight on the front, the name on the sides and lid, the ingredients and the details lines on the back (a pouch's back halves), the veg mark for India |
| Pack dieline | The cut line as one closed curve (magenta) and the fold and crease lines (amber, dash and dot) |
| Pack guides | Bleed edge (red), trim edge (black), the safe margin of every face (blue, dashed), the seals of a pouch as no-go zones (amber, dashed), the panel names (pink) and, with Front-of-pack zone, the country's zone with its name |

A pouch's safe area also keeps off the fin seals and the crimp seals.
The front-of-pack zones are planning guides placed on the front's safe
area, not regulations; the European Union and the UAE / GCC have none.

## Nutrition

Country: India, USA, Canada, United Kingdom, European Union, Australia /
New Zealand, UAE / GCC, Mexico, Chile or Singapore; the country's note
(from the labelling guides) shows under the form. Amounts are for one
serving; the serving size gives the amounts per 100 g.

Insert table draws the table as one group in one undo step ("Nutrition
table") and selects it. With Put it on the back panel (on at first) it
is centred on the box's back panel (a pouch's left half), else on the
page.

| Country | Table |
|---|---|
| USA, Canada | The facts panel: serving size, calories, % Daily Value; amounts rounded the FDA way (calories to 5 up to 50 and to 10 above, fats to 0.5 g under 5 g, sodium to 5 mg up to 140 mg and to 10 mg above, cholesterol to 5 mg, other amounts to 1 g); the daily values are those of the US label (fat 78 g, saturated fat 20 g, cholesterol 300 mg, sodium 2300 mg, carbohydrate 275 g, fibre 28 g, added sugars 50 g, vitamin D 20 mcg, calcium 1300 mg, iron 18 mg, potassium 4700 mg) and of the Canadian one (fat 75 g, saturated and trans 20 g, carbohydrate 275 g, fibre 28 g, sugars 100 g, sodium 2300 mg, potassium 3400 mg, calcium 1300 mg, iron 14 mg); the Canadian panel is in English and French |
| India | "NUTRITIONAL INFORMATION (Approx.)", per 100 g: energy, protein, carbohydrate, total and added sugars, total fat, saturated and trans fat, cholesterol, sodium |
| United Kingdom, European Union | Per 100 g: energy in kJ and kcal, fat, saturates, carbohydrate, sugars, fibre, protein, salt (sodium x 2.5) |
| Australia / New Zealand, UAE / GCC, Singapore | Per serving and per 100 g (the UAE / GCC and Singapore tables follow the guides' lists; Arabic text is added by hand) |
| Mexico, Chile | Spanish tables per 100 g |

Tables with columns show one column, per 100 g, unless Also a column per
serving is ticked. Energy in kilojoules is kilocalories x 4.184. The
sample amounts are to be replaced by the lab's; the guides are general
(Nov 2026) and the rules change, so the label is checked with the
regulator before final artwork.

## Export for print

Export (which layers go in the PDF): Artwork only, Artwork + cut and fold
lines, Artwork + cut, fold and guides, or Dieline only (cut and fold),
for the die maker. PDF standard (PDF/X-1a:2003, PDF/X-3:2003 or PDF/X-4,
X-4 at first), Bleed and the printers' marks (crop marks, bleed marks,
registration marks, colour bars, file information). Export for print
makes the box's layers print or not as the choice says (one undo step,
"Pack Studio"; a page without a box is left as it is), closes Pack
Studio and opens Export as PDF with that level, bleed and marks chosen;
everything else in that dialog is as in `publish-to-pdf.md`. Create box
again makes the dieline and guide layers not print once more.

## Checks

- `pack_studio::tests::the_default_carton_has_the_reference_trim_size`
- `pack_studio::tests::the_cut_line_closes_inside_the_trim_box`
- `pack_studio::tests::panels_lie_inside_the_trim_box_and_do_not_overlap`
- `pack_studio::tests::a_sleeve_has_no_flaps_and_a_tuck_end_carton_has_creases_for_them`
- `pack_studio::tests::sizes_that_cannot_be_drawn_are_kept_drawable`
- `pack_studio::tests::artwork_runs_over_the_bleed_on_the_trim_edges_only`
- `pack_studio::tests::the_guides_hold_what_is_switched_on`
- `pack_studio::tests::a_pouch_is_two_back_halves_round_a_front_with_seals_kept_clear`
- `pack_studio::tests::the_india_pouch_gets_a_veg_mark_and_the_table_follows_the_reference`
- `pack_studio::tests::the_fda_rounds_amounts_its_way`
- `pack_studio::tests::daily_values_are_shares_of_the_reference_amounts`
- `pack_studio::tests::per_100_g_scales_the_serving_and_energy_converts`
- `pack_studio::tests::every_country_makes_a_table_that_stays_in_its_width`
- `pack_studio::tests::the_us_panel_shows_calories_and_daily_values`
- `pack_studio::tests::a_box_is_made_in_layers_and_goes_in_one_undo_step`
- `pack_studio::tests::a_nutrition_table_is_one_selected_group`
- `pack_studio::tests::a_table_can_be_put_on_a_panel_of_the_box`
- `pack_studio::tests::the_dialog_starts_on_the_box_with_the_table_going_on_the_back`
- `pack_studio::tests::export_for_print_makes_the_chosen_layers_print`
- `pack_studio::tests::export_for_print_opens_the_pdf_dialog_with_the_level_and_marks`
