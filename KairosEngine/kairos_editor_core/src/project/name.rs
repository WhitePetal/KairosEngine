//! Name validation for file operations: the one table every create, rename and
//! move shares.
//!
//! Windows is the strictest target (reserved device names, trailing dots and
//! spaces, the alternate-data-stream colon), so the rules are written once here
//! and applied on every platform: a project that opens on Windows is the case
//! that matters, and a rule that only fires on one OS would let a project become
//! unopenable by moving it.
//!
//! This module is pure: it never touches the file system and never looks at the
//! tree. Sister-name collisions need the tree, so the reducer checks those; the
//! root is not renamable for the same reason (it has no sibling set).
//!
//! The messages are user-facing: they are what the panel shows in its status bar
//! or under the inline rename editor, so each one names the rule that failed.

/// Why a proposed name cannot become a project entry name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    /// Empty, or only whitespace.
    Empty,
    /// [`"."`] or [`".."`]: path components, not names.
    PathComponent,
    /// A path separator ([`/`] or [`\`]). The editor never accepts a whole path
    /// where a name belongs; moving is a separate action.
    Separator,
    /// A character Windows forbids in a file name: [`<>:"|?*`].
    Forbidden(char),
    /// A control character.
    Control(char),
    /// A Windows device name ([`CON`], [`PRN`], [`AUX`], [`NUL`], [`COM1..9`],
    /// [`LPT1..9`], [`CONIN$`], [`CONOUT$`]), with or without an extension.
    Reserved,
    /// A trailing dot or space: Windows silently strips it, so the file would
    /// come back under a different name than the one the user typed.
    TrailingDotOrSpace,
    /// Longer than [`MAX_NAME_BYTES`] bytes, the NTFS component limit.
    TooLong { bytes: usize },
}

impl NameError {
    /// The one-line message the panel shows.
    pub fn message(self) -> String {
        match self {
            Self::Empty => String::from("a name cannot be empty"),
            Self::PathComponent => String::from("a name cannot be . or .."),
            Self::Separator => String::from("a name cannot contain / or \\"),
            Self::Forbidden(character) => {
                format!("a name cannot contain {character:?}")
            }
            Self::Control(character) => {
                format!(
                    "a name cannot contain the control character {:#04x}",
                    u32::from(character)
                )
            }
            Self::Reserved => String::from("that is a reserved device name on Windows"),
            Self::TrailingDotOrSpace => String::from("a name cannot end with a dot or a space"),
            Self::TooLong { bytes } => {
                format!("a name is limited to {MAX_NAME_BYTES} bytes, not {bytes}")
            }
        }
    }
}

/// NTFS' limit for one path component, in bytes of UTF-8.
pub const MAX_NAME_BYTES: usize = 255;

/// The characters Windows forbids in a file name.
///
/// The list is data rather than seven match arms so [`validate`] can scan the
/// name once and report the first offender; the character named in the message
/// is then the one the user sees and fixes.
const FORBIDDEN_CHARACTERS: [char; 7] = ['<', '>', ':', '"', '|', '?', '*'];

/// Windows device names, lowercased; the stem comparison is case-insensitive.
///
/// [`COM1..=COM9`] and [`LPT1..=LPT9`] are spelled out on purpose: [`COM10`],
/// [`COM0`] and [`LPT0`] are ordinary names, and a prefix test would need a "the
/// rest is exactly one digit 1-9" branch — the same table with more ways to be
/// wrong. The two console aliases carry a trailing [`$`], which is an ordinary
/// character.
const RESERVED_NAMES: [&str; 24] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9", "conin$",
    "conout$",
];

/// The first character in [`name`] that Windows forbids, in name order.
///
/// In name order, not table order: with two offenders in the name, the message
/// names the one nearer the start, which is the one the user reads first.
fn forbidden_character(name: &str) -> Option<char> {
    name.chars()
        .find(|character| FORBIDDEN_CHARACTERS.contains(character))
}

/// Whether [`name`]'s stem names a Windows device.
///
/// The stem is the text before the first [`.`] with trailing spaces trimmed,
/// which is the rule Windows applies: [`CON.txt`], [`Nul.TXT`] and [`aux `] all
/// resolve to a device, while [`COM10`], [`console`] and [`conin`] (without the
/// [`$`]) do not. Only spaces are trimmed, not tabs: a tab is already
/// [`NameError::Control`], and trimming it here would hide which rule fired.
fn is_reserved(name: &str) -> bool {
    let stem_end = name.find('.').unwrap_or(name.len());
    let stem = name[..stem_end].trim_end_matches(' ');
    RESERVED_NAMES
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
}

/// Validate one proposed entry name.
///
/// This is the whole rule set: if it returns [`Ok`], the only thing left to check
/// is whether a sister entry already uses the name.
///
/// Checks run in [`NameError`]'s declaration order and the first match wins, so
/// that order is observable:
///
/// * whitespace-only names are [`NameError::Empty`] even when the whitespace is
///   a control character ([`"\t"`]): "empty" is the message the user can act on;
/// * a device name is [`NameError::Reserved`] even with a trailing space
///   ([`"aux "`]), because the stem is trimmed before the comparison — that is
///   also why [`"aux "`] never reaches [`NameError::TrailingDotOrSpace`];
/// * the byte limit is checked last, so an overlong name that also ends in a dot
///   reports the dot: the visible defect is the more useful message.
///
/// Everything else is accepted: non-ASCII names, interior spaces and dots,
/// leading dots and leading spaces.
pub fn validate(name: &str) -> Result<(), NameError> {
    // [`all`] is true for the empty string too, which is the point: an empty
    // name and a blank one are the same user mistake.
    if name
        .chars()
        .all(|character| character.is_ascii_whitespace())
    {
        return Err(NameError::Empty);
    }
    // Path components, not names: the editor's move action is what the user
    // wants here, and a rename cannot express it.
    if name == "." || name == ".." {
        return Err(NameError::PathComponent);
    }
    // A whole path where a name belongs: moving is a separate action, and a tree
    // path is not a rename.
    if name.contains('/') || name.contains('\\') {
        return Err(NameError::Separator);
    }
    // Forbidden characters before control characters: the sets do not overlap,
    // but a name can contain one of each, and the forbidden character is the
    // typo the user can see.
    if let Some(character) = forbidden_character(name) {
        return Err(NameError::Forbidden(character));
    }
    if let Some(character) = name.chars().find(|character| character.is_ascii_control()) {
        return Err(NameError::Control(character));
    }
    if is_reserved(name) {
        return Err(NameError::Reserved);
    }
    // Windows strips these instead of failing, so the entry would come back
    // under a different name than the one the user typed.
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(NameError::TrailingDotOrSpace);
    }
    // Last because names are compared and displayed before their length matters;
    // [`len`] is UTF-8 bytes, which is what NTFS counts.
    if name.len() > MAX_NAME_BYTES {
        return Err(NameError::TooLong { bytes: name.len() });
    }
    Ok(())
}

/// Whether [`name`] is already used by a sister entry.
///
/// Case-insensitive on every platform for the same reason the tree sorts that
/// way: a project authored on Linux must not become two files on Windows.
/// [`except`] is the entry being renamed, which must not collide with itself.
pub fn collides<'a>(
    name: &str,
    sisters: impl Iterator<Item = &'a str>,
    except: Option<&str>,
) -> bool {
    sisters
        .into_iter()
        .any(|sister| Some(sister) != except && sister.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_names_are_accepted() {
        for name in [
            "res",
            "Suzanne.png",
            ".gitignore",
            "a b",
            "模型",
            "v1.2",
            "a-b_c",
            "New Folder",
            "résumé",
            "a. b",
            " leading",
        ] {
            assert_eq!(validate(name), Ok(()), "{name:?} should be accepted");
        }
    }

    #[test]
    fn the_empty_rules_are_named() {
        assert_eq!(validate(""), Err(NameError::Empty));
        assert_eq!(validate("   "), Err(NameError::Empty));
        assert_eq!(validate("."), Err(NameError::PathComponent));
        assert_eq!(validate(".."), Err(NameError::PathComponent));
    }

    /// Rule 1 outranks rule 5: a tab and a newline are whitespace as well as
    /// control characters, and "empty" is the message that helps.
    #[test]
    fn whitespace_only_names_are_empty_not_control() {
        for name in ["\t  ", "\t", "\n", " \r\n "] {
            assert_eq!(validate(name), Err(NameError::Empty), "{name:?}");
        }
    }

    /// Only the exact strings are path components; a leading dot is ordinary.
    #[test]
    fn dot_is_a_path_component_not_a_name() {
        for name in [".", ".."] {
            assert_eq!(validate(name), Err(NameError::PathComponent), "{name:?}");
        }
        assert_eq!(validate(".gitignore"), Ok(()));
        assert_eq!(validate("..."), Err(NameError::TrailingDotOrSpace));
    }

    /// Rule 3 outranks rule 5: the separator is reported even though a control
    /// character also sits in the name.
    #[test]
    fn separators_are_rejected() {
        for name in ["a/b", "a\\b", "res/models", "/", "\\", "a/\u{0}"] {
            assert_eq!(validate(name), Err(NameError::Separator), "{name:?}");
        }
    }

    #[test]
    fn every_forbidden_character_is_reported() {
        for character in ['<', '>', ':', '"', '|', '?', '*'] {
            let name = format!("a{character}b");
            assert_eq!(
                validate(&name),
                Err(NameError::Forbidden(character)),
                "{name:?}"
            );
        }
        // Two offenders: the earlier character in the name is reported, not the
        // earlier one in the table.
        assert_eq!(validate("a?>b"), Err(NameError::Forbidden('?')));
        // Rule 4 outranks rule 5 even when the control character comes first.
        assert_eq!(validate("\u{0}?b"), Err(NameError::Forbidden('?')));
    }

    #[test]
    fn control_characters_are_reported() {
        for (name, control) in [
            ("a\u{0}b", '\u{0}'),
            ("a\u{1f}b", '\u{1f}'),
            ("a\u{7f}b", '\u{7f}'),
            ("a\nb", '\n'),
            ("a\tb", '\t'),
        ] {
            assert_eq!(validate(name), Err(NameError::Control(control)), "{name:?}");
        }
    }

    #[test]
    fn windows_device_names_are_reserved_in_any_case() {
        for name in [
            "CON", "con", "Con", "PRN", "prn", "AUX", "aux", "NUL", "nul", "COM1", "com9", "COM9",
            "LPT1", "lpt9", "LPT9", "CONIN$", "conin$", "CONOUT$", "conout$",
        ] {
            assert_eq!(validate(name), Err(NameError::Reserved), "{name:?}");
        }
    }

    /// The stem is what gets compared, so an extension does not hide the device.
    /// [`"aux "`] is the ordering case: the stem trims the trailing space, so the
    /// reserved rule wins over the trailing-space rule.
    #[test]
    fn device_names_are_reserved_through_extensions_and_spaces() {
        for name in [
            "con.txt",
            "Nul.TXT",
            "COM1.log",
            "lpt9.tar.gz",
            "CONIN$.meta",
            "aux ",
            "con. ",
        ] {
            assert_eq!(validate(name), Err(NameError::Reserved), "{name:?}");
        }
    }

    #[test]
    fn names_that_only_resemble_devices_are_accepted() {
        for name in [
            "COM10", "console", "prn2", "COM0", "lpt0", "conin", "conout", "nullable",
        ] {
            assert_eq!(validate(name), Ok(()), "{name:?}");
        }
    }

    #[test]
    fn trailing_dots_and_spaces_are_rejected() {
        for name in ["a.", "a ", "a. ", "a..", "a  ", "folder. "] {
            assert_eq!(
                validate(name),
                Err(NameError::TrailingDotOrSpace),
                "{name:?}"
            );
        }
        // [`"a. "`] reaches this rule because its stem ([`"a"`]) is not a device
        // name, and interior dots and spaces are ordinary.
        assert_eq!(validate("a .b"), Ok(()));
    }

    #[test]
    fn the_limit_is_255_utf8_bytes() {
        let ascii_at_limit = "a".repeat(MAX_NAME_BYTES);
        assert_eq!(ascii_at_limit.len(), 255);
        assert_eq!(validate(&ascii_at_limit), Ok(()));

        let ascii_over = "a".repeat(MAX_NAME_BYTES + 1);
        assert_eq!(
            validate(&ascii_over),
            Err(NameError::TooLong { bytes: 256 })
        );

        // NTFS counts bytes, not characters: 85 three-byte characters are 255
        // bytes and fit; 86 are 258 bytes and do not.
        let multibyte_at_limit = "模".repeat(85);
        assert_eq!(multibyte_at_limit.len(), MAX_NAME_BYTES);
        assert_eq!(validate(&multibyte_at_limit), Ok(()));

        let multibyte_over = "模".repeat(86);
        assert_eq!(multibyte_over.len(), 258);
        assert_eq!(
            validate(&multibyte_over),
            Err(NameError::TooLong { bytes: 258 })
        );
    }

    /// Rule 8 is last: a name can be overlong *and* otherwise wrong, and the
    /// character rule wins so the user sees a defect, not only a byte count.
    #[test]
    fn the_byte_limit_is_checked_last() {
        let mut overlong_dot = "a".repeat(MAX_NAME_BYTES + 1);
        overlong_dot.push('.');
        assert_eq!(validate(&overlong_dot), Err(NameError::TrailingDotOrSpace));

        let mut overlong_reserved = String::from("con.");
        overlong_reserved.push_str(&"a".repeat(MAX_NAME_BYTES));
        assert_eq!(validate(&overlong_reserved), Err(NameError::Reserved));
    }

    #[test]
    fn sisters_collide_case_insensitively() {
        let sisters = ["res", "Suzanne.png"];
        assert!(collides("RES", sisters.into_iter(), None));
        assert!(!collides("res", sisters.into_iter(), Some("res")));
        assert!(!collides("models", sisters.into_iter(), None));
    }
}
