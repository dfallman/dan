pub enum Message {
    ToolbarPrefix,
    DirtyFlag,
    HelpCommandKey,
    SelectionModeLabel,
    ModeLabelEditing,
    FilenameLabel(String),
    LineCol(usize, usize),
    Version(String, String),
    HelpTitle,
    HelpShortcutSave,
    HelpShortcutSaveAs,
    HelpShortcutQuit,
    HelpShortcutUndo,
    HelpShortcutRedo,
    HelpShortcutCopy,
    HelpShortcutCut,
    HelpShortcutPaste,
    HelpShortcutFind,
    HelpShortcutGoto,
    HelpShortcutDuplicate,
    HelpShortcutDelete,
    HelpShortcutWrap,
    HelpShortcutLint,
    HelpShortcutComment,
    HelpShortcutSyntax,
    HelpShortcutWhitespace,
    HelpShortcutPalette,
    HelpShortcutNewBuffer,
    HelpShortcutHelp,
    MatchFraction(usize, usize),
    ZeroMatches,
    SearchShortcuts,
    SearchReplaceShortcuts,
    ReplaceShortcuts,
    PromptReplaceWith,
    PromptReplaceStep,
    PromptGoToLine,
    PromptGoToLineHint(usize),
    PromptRecoverTitle,
    PromptRecoverMsg,
    PromptSaveAs,
    PromptConfirmOverwrite,
    PromptSaveAsShortcuts,
    PromptQuitWarning,
    PromptQuitMsg,
    PromptSearch,
    PromptClipLeft,
    PromptClipRight,
    StatusMessage(String),
    InfoBannerLabel,
    InfoBannerIndentTabs,
    InfoBannerIndentSpaces(usize),
    InfoBannerBody(String),
    EscToClose,
    PalettePlaceholder,
    PaletteSectionBuffers,
    PaletteSectionFiles,
    PaletteSectionCommands,
    PaletteFooterHints,
    PaletteFooterCloseHints,
    PaletteResultCount(usize, usize),
    PaletteIndexingSuffix,
    PaletteNoMatches,
}

pub trait Locale: Send + Sync {
    fn translate(&self, msg: Message) -> String;
}

pub struct EnglishLocale;

impl Locale for EnglishLocale {
    fn translate(&self, msg: Message) -> String {
        match msg {
            Message::ToolbarPrefix => "▌".to_string(),
            Message::DirtyFlag => "●".to_string(),
            Message::HelpCommandKey => "^H Help".to_string(), // we might just keep HelpTitle spaces? "Help"
            Message::SelectionModeLabel => "Select".to_string(),
            Message::ModeLabelEditing => "Edit".to_string(),
            Message::FilenameLabel(name) => name,
            Message::LineCol(ln, col) => format!("Ln {:2}, Col {:2}", ln, col),
            Message::Version(ver, hash) => format!("· Dan v{} ({})", ver, hash),
            Message::HelpTitle => "Help".to_string(),
            Message::HelpShortcutSave => "Save".to_string(),
            Message::HelpShortcutSaveAs => "Save as".to_string(),
            Message::HelpShortcutQuit => "Quit".to_string(),
            Message::HelpShortcutUndo => "Undo".to_string(),
            Message::HelpShortcutRedo => "Redo".to_string(),
            Message::HelpShortcutCopy => "Copy".to_string(),
            Message::HelpShortcutCut => "Cut".to_string(),
            Message::HelpShortcutPaste => "Paste".to_string(),
            Message::HelpShortcutFind => "Search & replace".to_string(),
            Message::HelpShortcutGoto => "Goto".to_string(),
            Message::HelpShortcutDuplicate => "Duplicate".to_string(),
            Message::HelpShortcutDelete => "Delete".to_string(),
            Message::HelpShortcutWrap => "Wrap".to_string(),
            Message::HelpShortcutLint => "Lint".to_string(),
            Message::HelpShortcutComment => "Comment".to_string(),
            Message::HelpShortcutSyntax => "Syntax highlight".to_string(),
            Message::HelpShortcutWhitespace => "Whitespace".to_string(),
            Message::HelpShortcutPalette => "Palette".to_string(),
            Message::HelpShortcutNewBuffer => "New buffer".to_string(),
            Message::HelpShortcutHelp => "Help".to_string(),
            Message::MatchFraction(idx, total) => format!("{}/{} matches", idx, total),
            Message::ZeroMatches => "0 matches".to_string(),
            Message::SearchShortcuts => "^G Next, ⏎ Select, Esc Close".to_string(),
            Message::SearchReplaceShortcuts => "^G Next, ^R Replace, ⏎ Select, Esc Close".to_string(),
            Message::ReplaceShortcuts => "⏎ Replace, Esc Close".to_string(),
            Message::PromptReplaceWith => "Replace with:".to_string(),
            Message::PromptReplaceStep => "Replace? ^Y Yes, ^N No, ^A All, Esc Close".to_string(),
            Message::PromptGoToLine => "Goto row:".to_string(),
            Message::PromptGoToLineHint(lines) => format!("(1-{})", lines),
            Message::PromptRecoverTitle => "Recovery".to_string(),
            Message::PromptRecoverMsg => "Recovery swap file detected. Restore unsaved changes? ^Y Yes, ^N No, ^Q or Esc to quit".to_string(),
            Message::PromptSaveAs => "Save as:".to_string(),
            Message::PromptConfirmOverwrite => "File exists. ^O Overwrite, Esc Cancel".to_string(),
            Message::PromptSaveAsShortcuts => "Type path, ⏎ Save, Esc Cancel".to_string(),
            Message::PromptQuitWarning => "Quit warning:".to_string(),
            Message::PromptQuitMsg => "Unsaved changes. ^S Save and quit, ^F Force quit without saving, Esc Cancel".to_string(),
            Message::PromptSearch => "Search:".to_string(),
            Message::PromptClipLeft => "←".to_string(),
            Message::PromptClipRight => "→".to_string(),
            Message::StatusMessage(msg) => msg,
            Message::InfoBannerLabel => "Info:".to_string(),
            Message::InfoBannerIndentTabs => "tabs".to_string(),
            Message::InfoBannerIndentSpaces(n) => format!("{} spaces", n),
            Message::InfoBannerBody(desc) => {
                format!(" Sniffer detected indent using {}, overriding default settings", desc)
            }
            Message::EscToClose => "Esc Close".to_string(),
            Message::PalettePlaceholder => {
                "Search buffers, files, and commands…".to_string()
            }
            Message::PaletteSectionBuffers => "Buffers".to_string(),
            Message::PaletteSectionFiles => "Files".to_string(),
            Message::PaletteSectionCommands => "Commands".to_string(),
            Message::PaletteFooterHints => " ↑↓ navigate  ↵ open  esc close".to_string(),
            Message::PaletteFooterCloseHints => {
                " ^S save  ^D discard  esc cancel".to_string()
            }
            Message::PaletteResultCount(shown, total) => format!("{} of {}", shown, total),
            Message::PaletteIndexingSuffix => " · indexing…".to_string(),
            Message::PaletteNoMatches => "No matches".to_string(),
        }
    }
}

/// Swedish UI strings — second locale to keep `Message` / `Locale` honest
/// (layout widths, plural forms, and English-shaped assumptions).
pub struct SwedishLocale;

impl Locale for SwedishLocale {
    fn translate(&self, msg: Message) -> String {
        match msg {
            Message::ToolbarPrefix => "▌".to_string(),
            Message::DirtyFlag => "●".to_string(),
            Message::HelpCommandKey => "^H Hjälp".to_string(),
            Message::SelectionModeLabel => "Markera".to_string(),
            Message::ModeLabelEditing => "Redigera".to_string(),
            Message::FilenameLabel(name) => name,
            Message::LineCol(ln, col) => format!("Rad {:2}, Kol {:2}", ln, col),
            Message::Version(ver, hash) => format!("· Dan v{} ({})", ver, hash),
            Message::HelpTitle => "Hjälp".to_string(),
            Message::HelpShortcutSave => "Spara".to_string(),
            Message::HelpShortcutSaveAs => "Spara som".to_string(),
            Message::HelpShortcutQuit => "Avsluta".to_string(),
            Message::HelpShortcutUndo => "Ångra".to_string(),
            Message::HelpShortcutRedo => "Gör om".to_string(),
            Message::HelpShortcutCopy => "Kopiera".to_string(),
            Message::HelpShortcutCut => "Klipp ut".to_string(),
            Message::HelpShortcutPaste => "Klistra in".to_string(),
            Message::HelpShortcutFind => "Sök & ersätt".to_string(),
            Message::HelpShortcutGoto => "Gå till".to_string(),
            Message::HelpShortcutDuplicate => "Duplicera".to_string(),
            Message::HelpShortcutDelete => "Radera".to_string(),
            Message::HelpShortcutWrap => "Radbryt".to_string(),
            Message::HelpShortcutLint => "Formatera".to_string(),
            Message::HelpShortcutComment => "Kommentera".to_string(),
            Message::HelpShortcutSyntax => "Syntaxfärg".to_string(),
            Message::HelpShortcutWhitespace => "Blanksteg".to_string(),
            Message::HelpShortcutPalette => "Palett".to_string(),
            Message::HelpShortcutNewBuffer => "Ny buffert".to_string(),
            Message::HelpShortcutHelp => "Hjälp".to_string(),
            Message::MatchFraction(idx, total) => format!("{}/{} träffar", idx, total),
            Message::ZeroMatches => "0 träffar".to_string(),
            Message::SearchShortcuts => "^G Nästa, ⏎ Markera, Esc Stäng".to_string(),
            Message::SearchReplaceShortcuts => {
                "^G Nästa, ^R Ersätt, ⏎ Markera, Esc Stäng".to_string()
            }
            Message::ReplaceShortcuts => "⏎ Ersätt, Esc Stäng".to_string(),
            Message::PromptReplaceWith => "Ersätt med:".to_string(),
            Message::PromptReplaceStep => {
                "Ersätt? ^Y Ja, ^N Nej, ^A Alla, Esc Stäng".to_string()
            }
            Message::PromptGoToLine => "Gå till rad:".to_string(),
            Message::PromptGoToLineHint(lines) => format!("(1-{})", lines),
            Message::PromptRecoverTitle => "Återställning".to_string(),
            Message::PromptRecoverMsg => {
                "Återställningsfil hittades. Återställ osparade ändringar? ^Y Ja, ^N Nej, ^Q eller Esc för att avsluta".to_string()
            }
            Message::PromptSaveAs => "Spara som:".to_string(),
            Message::PromptConfirmOverwrite => {
                "Filen finns. ^O Skriv över, Esc Avbryt".to_string()
            }
            Message::PromptSaveAsShortcuts => "Skriv sökväg, ⏎ Spara, Esc Avbryt".to_string(),
            Message::PromptQuitWarning => "Avsluta:".to_string(),
            Message::PromptQuitMsg => {
                "Osparade ändringar. ^S Spara och avsluta, ^F Avsluta utan att spara, Esc Avbryt".to_string()
            }
            Message::PromptSearch => "Sök:".to_string(),
            Message::PromptClipLeft => "←".to_string(),
            Message::PromptClipRight => "→".to_string(),
            Message::StatusMessage(msg) => msg,
            Message::InfoBannerLabel => "Info:".to_string(),
            Message::InfoBannerIndentTabs => "tabbar".to_string(),
            Message::InfoBannerIndentSpaces(n) => format!("{} mellanslag", n),
            Message::InfoBannerBody(desc) => {
                format!(" Sniffer upptäckte indrag med {}, åsidosätter standardinställningar", desc)
            }
            Message::EscToClose => "Esc Stäng".to_string(),
            Message::PalettePlaceholder => {
                "Sök buffertar, filer och kommandon…".to_string()
            }
            Message::PaletteSectionBuffers => "Buffertar".to_string(),
            Message::PaletteSectionFiles => "Filer".to_string(),
            Message::PaletteSectionCommands => "Kommandon".to_string(),
            Message::PaletteFooterHints => " ↑↓ navigera  ↵ öppna  esc stäng".to_string(),
            Message::PaletteFooterCloseHints => {
                " ^S spara  ^D kasta  esc avbryt".to_string()
            }
            Message::PaletteResultCount(shown, total) => format!("{} av {}", shown, total),
            Message::PaletteIndexingSuffix => " · indexerar…".to_string(),
            Message::PaletteNoMatches => "Inga träffar".to_string(),
        }
    }
}

#[cfg(test)]
/// Every `Message` variant used by chrome — keep in sync when adding variants.
fn all_message_samples() -> Vec<Message> {
    vec![
        Message::ToolbarPrefix,
        Message::DirtyFlag,
        Message::HelpCommandKey,
        Message::SelectionModeLabel,
        Message::ModeLabelEditing,
        Message::FilenameLabel("file.rs".into()),
        Message::LineCol(1, 1),
        Message::Version("0.1".into(), "abc".into()),
        Message::HelpTitle,
        Message::HelpShortcutSave,
        Message::HelpShortcutSaveAs,
        Message::HelpShortcutQuit,
        Message::HelpShortcutUndo,
        Message::HelpShortcutRedo,
        Message::HelpShortcutCopy,
        Message::HelpShortcutCut,
        Message::HelpShortcutPaste,
        Message::HelpShortcutFind,
        Message::HelpShortcutGoto,
        Message::HelpShortcutDuplicate,
        Message::HelpShortcutDelete,
        Message::HelpShortcutWrap,
        Message::HelpShortcutLint,
        Message::HelpShortcutComment,
        Message::HelpShortcutSyntax,
        Message::HelpShortcutWhitespace,
        Message::HelpShortcutPalette,
        Message::HelpShortcutNewBuffer,
        Message::HelpShortcutHelp,
        Message::MatchFraction(1, 3),
        Message::ZeroMatches,
        Message::SearchShortcuts,
        Message::SearchReplaceShortcuts,
        Message::ReplaceShortcuts,
        Message::PromptReplaceWith,
        Message::PromptReplaceStep,
        Message::PromptGoToLine,
        Message::PromptGoToLineHint(10),
        Message::PromptRecoverTitle,
        Message::PromptRecoverMsg,
        Message::PromptSaveAs,
        Message::PromptConfirmOverwrite,
        Message::PromptSaveAsShortcuts,
        Message::PromptQuitWarning,
        Message::PromptQuitMsg,
        Message::PromptSearch,
        Message::PromptClipLeft,
        Message::PromptClipRight,
        Message::StatusMessage("ok".into()),
        Message::EscToClose,
        Message::PalettePlaceholder,
        Message::PaletteSectionBuffers,
        Message::PaletteSectionFiles,
        Message::PaletteSectionCommands,
        Message::PaletteFooterHints,
        Message::PaletteFooterCloseHints,
        Message::PaletteResultCount(2, 5),
        Message::PaletteIndexingSuffix,
        Message::PaletteNoMatches,
        Message::InfoBannerLabel,
        Message::InfoBannerIndentTabs,
        Message::InfoBannerIndentSpaces(4),
        Message::InfoBannerBody("tabs".into()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swedish_covers_every_message_variant() {
        let sv = SwedishLocale;
        for msg in all_message_samples() {
            let s = sv.translate(msg);
            assert!(!s.is_empty(), "Swedish translation must be non-empty");
        }
    }

    #[test]
    fn english_and_swedish_differ_on_chrome_labels() {
        let en = EnglishLocale;
        let sv = SwedishLocale;
        // Spot-check that the second locale is not a copy of English.
        assert_ne!(
            en.translate(Message::HelpTitle),
            sv.translate(Message::HelpTitle)
        );
        assert_ne!(
            en.translate(Message::ModeLabelEditing),
            sv.translate(Message::ModeLabelEditing)
        );
        assert_ne!(
            en.translate(Message::PaletteNoMatches),
            sv.translate(Message::PaletteNoMatches)
        );
        // Pass-through messages must stay identical.
        assert_eq!(
            en.translate(Message::FilenameLabel("x".into())),
            sv.translate(Message::FilenameLabel("x".into()))
        );
        assert_eq!(
            en.translate(Message::StatusMessage("hi".into())),
            sv.translate(Message::StatusMessage("hi".into()))
        );
    }

    #[test]
    fn swedish_line_col_is_wider_than_english_shape() {
        // Guards against English-only width assumptions in chrome layout:
        // "Rad"/"Kol" vs "Ln"/"Col" change status-bar geometry.
        let en = EnglishLocale.translate(Message::LineCol(12, 34));
        let sv = SwedishLocale.translate(Message::LineCol(12, 34));
        assert!(sv.chars().count() >= en.chars().count());
        assert!(sv.contains("Rad"));
        assert!(sv.contains("Kol"));
    }

    #[test]
    fn sniff_info_banner_english_shape() {
        let en = EnglishLocale;
        assert_eq!(en.translate(Message::InfoBannerLabel), "Info:");
        assert_eq!(en.translate(Message::InfoBannerIndentTabs), "tabs");
        assert_eq!(en.translate(Message::InfoBannerIndentSpaces(8)), "8 spaces");
        assert_eq!(
            en.translate(Message::InfoBannerBody("4 spaces".into())),
            " Sniffer detected indent using 4 spaces, overriding default settings"
        );
    }
}
