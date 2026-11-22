//! The fonts a PDF embeds, for text that stays text. A PDF's text is read
//! as text (so it can be edited) and drawn with the installed font of the
//! same name; with that font missing a stand-in drew it, in other widths,
//! and the page changed its look. The font programs the file embeds are
//! installed for the session instead, so the text keeps its own font and
//! its widths. An embedded font is a subset as a rule: a letter the file
//! does not use is not in it and comes from the stand-in. A font that is
//! installed is left to draw its text as before. See
//! docs/behavior/pdf-import.md.

use tracedraw_io::pdf_import::EmbeddedFont;

/// Install the embedded fonts of the families that are not installed and
/// return those families. The drawing keeps its font names and saved
/// files do too: on a computer without the font a saved drawing draws
/// with the stand-in again.
pub fn use_embedded_fonts(fonts: &[EmbeddedFont]) -> Vec<String> {
    let system = tracedraw_text::fonts();
    let mut missing: Vec<&str> = Vec::new();
    for f in fonts {
        if !system.has_family(&f.family) && !missing.contains(&f.family.as_str()) {
            missing.push(&f.family);
        }
    }
    // Every style of a family goes in before the family is looked at again.
    let mut added: Vec<(&str, Vec<String>)> = Vec::new();
    for family in &missing {
        let names = system.add_fonts(
            fonts
                .iter()
                .filter(|f| f.family == *family)
                .map(|f| f.data.clone())
                .collect(),
        );
        added.push((family, names));
    }
    let mut used = Vec::new();
    for (family, names) in added {
        if system.has_family(family) {
            used.push(family.to_string());
        } else if let Some(name) = names.first() {
            // The program names itself differently from the PDF.
            system.set_substitute(family, Some(name));
            used.push(family.to_string());
        }
    }
    used
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A family that is not installed takes the embedded font; one that
    /// is installed is left alone.
    #[test]
    fn a_missing_family_takes_its_embedded_font() {
        let data = include_bytes!("../../../crates/text/testdata/Quicksand-VF.ttf").to_vec();
        let font = |family: &str| EmbeddedFont {
            family: family.into(),
            bold: false,
            italic: false,
            data: data.clone(),
        };
        let used = use_embedded_fonts(&[font("Pdf Only Family"), font("Pdf Only Family")]);
        assert_eq!(used, vec!["Pdf Only Family".to_string()]);
        assert!(tracedraw_text::fonts().has_family("Quicksand"));
        // The family the font installed under is installed: not taken again.
        assert!(use_embedded_fonts(&[font("Quicksand")]).is_empty());
        assert!(use_embedded_fonts(&[]).is_empty());
    }
}
