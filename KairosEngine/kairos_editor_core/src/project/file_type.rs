//! Asset families: what one file's extension decides about it.
//!
//! The plan's M5 seam, pulled forward into M3 because M3 needs three of its
//! answers: which sidecars travel with a file ([`FileType::companion`], the
//! rename/move/delete cluster rule), whether a delete asks first
//! ([`FileType::delete_confirm`], D5), and what "New ..." offers
//! ([`CreateEntry`]). Building the table now means M3's file operations are
//! written against data instead of against three hardcoded branches that M5
//! would then rewrite.
//!
//! The registry is **display and policy only**: it never opens a file, never
//! names a path and never reads the disk. Opening ([`OpenWith`]) and persistent
//! identity ([`referenceable`], M7) are added in their own milestones, when they
//! have a consumer.
//!
//! Two tables, on purpose:
//!
//! * [`FileType`] is keyed by extension: one file's family decides its row
//!   label, icon, whether it hides, and whether it moves with a sidecar.
//! * [`CreateEntry`] is keyed by *intent*: the "Create" menu is a list of things
//!   a user makes ("Folder", "Document > Markdown"), which is not the same axis
//!   as the file format. A creatable entry names the extension it produces, so
//!   the created row's display still comes from [`FileType`].

use std::fmt;

use super::fs::CreateKind;

/// One family of project files, keyed by extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileType {
    /// Stable id: what a future open/edit handler switches on ("texture",
    /// "material", ...). The extension is [`Self::label`]'s job, not the id's: a
    /// later type can own several extensions.
    pub id: &'static str,
    /// The row's right-aligned type label ("png", "mat"), which is the file's
    /// extension for every type in today's table.
    pub label: &'static str,
    /// Every extension this family answers to, lower case.
    pub extensions: &'static [&'static str],
    /// Row icon file name inside the skin's type-icon root, e.g. `texture.png`.
    ///
    /// The table never spells a full path: the root is appearance and lives in
    /// the panel skin (`ProjectStyle::type_icon_root`), so the dark and light
    /// skins can point at different folders without a second table. Empty means
    /// "the generic file icon": a family whose artwork has not been committed
    /// yet still draws.
    pub icon: &'static str,
    /// A sidecar of another file (today only [`.meta`]): kept out of the tree
    /// unless hidden files are shown, and moved/renamed/deleted with its owner.
    pub companion: bool,
    /// The row icon is the file's own content, decoded by the host, instead of
    /// the family's [`Self::icon`]. True for textures: an asset browser that
    /// shows a generic glyph for every image tells the user nothing, while the
    /// image itself is the one useful preview a tree row can carry.
    pub thumbnail: bool,
    /// Whether deleting this family asks for confirmation first (D5). Only an
    /// explicit false turns the dialog off; see [`FileTypeRegistry::delete_confirm`].
    pub delete_confirm: bool,
}

impl FileType {
    fn matches(&self, extension: &str) -> bool {
        self.extensions
            .iter()
            .any(|known| known.eq_ignore_ascii_case(extension))
    }
}

/// One entry of the Project panel's "Create" menu.
///
/// [`menu`] is the display path under "Create": empty draws the entry directly
/// below it ("Create > Folder"), a path draws nested submenus
/// ("Create > Document > Markdown"). M5's per-type tasks add rows here; the
/// panel's menu code never changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreateEntry {
    pub menu: &'static [&'static str],
    pub label: &'static str,
    /// Initial name for the new entry; the panel starts an inline rename on it.
    pub default_stem: &'static str,
    pub target: CreateKind,
}

/// The project's file-type table plus its create entries.
///
/// Cheap to copy (two static slices), so [`ProjectState`](super::ProjectState)
/// owns one by value and every pure function takes it by reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileTypeRegistry {
    types: &'static [FileType],
    create: &'static [CreateEntry],
}

impl Default for FileTypeRegistry {
    fn default() -> Self {
        Self::builtin()
    }
}

impl FileTypeRegistry {
    /// The table this build ships.
    pub const fn builtin() -> Self {
        Self {
            types: BUILTIN_TYPES,
            create: BUILTIN_CREATE,
        }
    }

    /// A registry from explicit tables: the M5 acceptance test registers a fake
    /// type this way and asserts no panel or tree code had to change.
    pub const fn new(types: &'static [FileType], create: &'static [CreateEntry]) -> Self {
        Self { types, create }
    }

    pub fn types(&self) -> impl Iterator<Item = &FileType> + '_ {
        self.types.iter()
    }

    /// The family a file name belongs to, if any.
    ///
    /// Matching is case-insensitive (a project moved between platforms must keep
    /// one meaning) and the **longest registered extension wins**, so a future
    /// [`tar.gz`] family beats a [`gz`] one. A leading dot is part of the name,
    /// not an extension: [`.gitignore`] has no family (the same rule the row
    /// suffix uses).
    pub fn lookup(&self, name: &str) -> Option<&FileType> {
        // Every dot starts one candidate suffix; the leftmost dot gives the
        // longest extension, which is what "longest wins" means.
        let bytes = name.as_bytes();
        for (index, byte) in bytes.iter().enumerate() {
            if *byte != b'.' || index == 0 || index + 1 >= bytes.len() {
                continue;
            }
            let suffix = &name[index + 1..];
            if let Some(found) = self.types.iter().find(|kind| kind.matches(suffix)) {
                return Some(found);
            }
        }
        None
    }

    /// The row's type label: the family's, or the bare extension for a file no
    /// family claims. [`None`] only for names without an extension.
    pub fn label<'a>(&self, name: &'a str) -> Option<&'a str> {
        match self.lookup(name) {
            Some(kind) => Some(kind.label),
            None => extension(name),
        }
    }

    /// The row icon file name for a file, or [`None`] for the panel's generic
    /// file icon. The caller joins it with its skin's type-icon root.
    pub fn icon(&self, name: &str) -> Option<&'static str> {
        self.lookup(name)
            .map(|kind| kind.icon)
            .filter(|icon| !icon.is_empty())
    }

    /// Whether the row icon for a file is its own decoded content rather than
    /// the family icon (textures today). The host's thumbnail cache answers the
    /// actual image; this is only the routing rule.
    pub fn thumbnail(&self, name: &str) -> bool {
        self.lookup(name).is_some_and(|kind| kind.thumbnail)
    }

    /// Whether the name is a sidecar of another file.
    pub fn is_companion(&self, name: &str) -> bool {
        self.lookup(name).is_some_and(|kind| kind.companion)
    }

    /// Whether deleting the file asks first. An unknown file type confirms: D5
    /// makes confirmation the default and only an explicit type rule relaxes it.
    pub fn delete_confirm(&self, name: &str) -> bool {
        self.lookup(name).is_none_or(|kind| kind.delete_confirm)
    }

    /// The extensions of companion sidecars ("meta"), in table order.
    ///
    /// The create/rename/move executor uses this to carry a file's sidecars with
    /// it (G2/G3): the rule is the type table's, so a future `.uid` sidecar is one
    /// row here and no host code changes.
    pub fn companion_extensions(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.types
            .iter()
            .filter(|kind| kind.companion)
            .flat_map(|kind| kind.extensions.iter().copied())
    }

    /// Is [`name`] shown by default?
    pub fn is_hidden(&self, name: &str) -> bool {
        name.starts_with('.') || self.is_companion(name)
    }

    /// The "Create" menu, in display order.
    pub fn create_entries(&self) -> &'static [CreateEntry] {
        self.create
    }
}

impl fmt::Display for FileTypeRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} file types, {} create entries",
            self.types.len(),
            self.create.len()
        )
    }
}

/// The extension of a file name, or [`None`] when it has none.
///
/// The single definition of "extension" for the whole project slice: a leading
/// dot is part of the name ([`.gitignore`]) and the text after the last dot must
/// be non-empty for the stem to be non-empty.
pub fn extension(name: &str) -> Option<&str> {
    let (stem, extension) = name.rsplit_once('.')?;
    if stem.is_empty() || extension.is_empty() {
        return None;
    }
    Some(extension)
}

/// The types this build ships (see the plan's section 2.8).
///
/// The list mirrors the legacy asset kinds (one row per family the old tree
/// displayed) so M3 and M5 do not re-decide what a [`.mat`] is. The icon files
/// arrive with M3b's Tuanjie extraction; until then a row with a missing file
/// simply draws nothing and the panel's generic file icon is used for files with
/// no artwork.
const BUILTIN_TYPES: &[FileType] = &[
    FileType {
        id: "texture",
        label: "png",
        extensions: &["png"],
        icon: "texture.png",
        companion: false,
        thumbnail: true,
        delete_confirm: true,
    },
    FileType {
        id: "mesh",
        label: "glb",
        extensions: &["glb"],
        icon: "mesh.png",
        companion: false,
        thumbnail: false,
        delete_confirm: true,
    },
    FileType {
        id: "material",
        label: "mat",
        extensions: &["mat"],
        icon: "material.png",
        companion: false,
        thumbnail: false,
        delete_confirm: true,
    },
    FileType {
        id: "audio",
        label: "audio",
        extensions: &["ogg", "wav", "audio"],
        icon: "audio.png",
        companion: false,
        thumbnail: false,
        delete_confirm: true,
    },
    FileType {
        id: "shader",
        label: "wgsl",
        extensions: &["wgsl"],
        icon: "shader.png",
        companion: false,
        thumbnail: false,
        delete_confirm: true,
    },
    FileType {
        id: "script",
        label: "rs",
        extensions: &["rs"],
        icon: "script.png",
        companion: false,
        thumbnail: false,
        delete_confirm: true,
    },
    FileType {
        id: "document",
        label: "md",
        extensions: &["md"],
        icon: "document.png",
        companion: false,
        thumbnail: false,
        delete_confirm: true,
    },
    FileType {
        id: "font",
        label: "ttf",
        extensions: &["ttf", "otf"],
        icon: "font.png",
        companion: false,
        thumbnail: false,
        delete_confirm: true,
    },
    FileType {
        id: "companion",
        label: "meta",
        extensions: &["meta"],
        icon: "",
        companion: true,
        thumbnail: false,
        delete_confirm: true,
    },
];

/// The Create menu this build ships.
///
/// Two entries for M3: the folder, and the one document template the design
/// review approved. [`.toml`] deliberately has no entry: it is a serialisation
/// format that some other system owns, not something a user creates bare. The
/// per-type entries (shader, script, material, ...) arrive with M5's type tasks,
/// one row each.
const BUILTIN_CREATE: &[CreateEntry] = &[
    CreateEntry {
        menu: &[],
        label: "Folder",
        default_stem: "New Folder",
        target: CreateKind::Folder,
    },
    CreateEntry {
        menu: &["Document"],
        label: "Markdown",
        default_stem: "New Document",
        target: CreateKind::File {
            extension: "md",
            contents: "# ",
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn builtin() -> FileTypeRegistry {
        FileTypeRegistry::builtin()
    }

    #[test]
    fn extensions_match_case_insensitively() {
        let registry = builtin();
        assert_eq!(
            registry.lookup("Suzanne.PNG").map(|kind| kind.id),
            Some("texture")
        );
        assert_eq!(
            registry.lookup("readme.MD").map(|kind| kind.id),
            Some("document")
        );
        assert_eq!(
            registry.lookup("shader.WGSL").map(|kind| kind.id),
            Some("shader")
        );
    }

    #[test]
    fn one_family_answers_to_several_extensions() {
        let registry = builtin();
        for name in ["score.ogg", "sine.wav", "blip.audio"] {
            assert_eq!(
                registry.lookup(name).map(|kind| kind.id),
                Some("audio"),
                "{name}"
            );
        }
        for name in ["Inter-Regular.otf", "SmileySans.ttf"] {
            assert_eq!(
                registry.lookup(name).map(|kind| kind.id),
                Some("font"),
                "{name}"
            );
        }
    }

    /// "Longest registered extension wins": a fake tar.gz family beats gz, which
    /// is what lets a format request a compound extension later without touching
    /// the lookup.
    #[test]
    fn the_longest_registered_extension_wins() {
        static FAKE: [FileType; 2] = [
            FileType {
                id: "archive",
                label: "gz",
                extensions: &["gz"],
                icon: "",
                companion: false,
                thumbnail: false,
                delete_confirm: true,
            },
            FileType {
                id: "bundle",
                label: "tar.gz",
                extensions: &["tar.gz"],
                icon: "",
                companion: false,
                thumbnail: false,
                delete_confirm: true,
            },
        ];
        static NONE: [CreateEntry; 0] = [];
        let registry = FileTypeRegistry::new(&FAKE, &NONE);
        assert_eq!(
            registry.lookup("bundle.tar.gz").map(|kind| kind.id),
            Some("bundle")
        );
        assert_eq!(
            registry.lookup("plain.gz").map(|kind| kind.id),
            Some("archive")
        );
    }

    #[test]
    fn a_leading_dot_is_part_of_the_name() {
        assert_eq!(extension(".gitignore"), None);
        assert_eq!(extension("archive.tar.gz"), Some("gz"));
        assert_eq!(extension("no-extension"), None);
        assert_eq!(extension("trailing."), None);
        assert_eq!(builtin().lookup(".gitignore"), None);
        assert_eq!(builtin().label(".gitignore"), None);
    }

    #[test]
    fn companions_are_recognised_and_hidden() {
        let registry = builtin();
        assert!(registry.is_companion("Suzanne.glb.meta"));
        assert!(registry.is_companion("material.mat.meta"));
        assert!(!registry.is_companion("material.mat"));
        assert!(registry.is_hidden(".git"));
        assert!(registry.is_hidden("Suzanne.glb.meta"));
        assert!(!registry.is_hidden("Suzanne.glb"));
    }

    #[test]
    fn unknown_files_fall_back_to_the_generic_row() {
        let registry = builtin();
        assert_eq!(registry.lookup("notes.xyz"), None);
        assert_eq!(registry.icon("notes.xyz"), None);
        assert_eq!(registry.label("notes.xyz"), Some("xyz"));
        assert!(
            registry.delete_confirm("notes.xyz"),
            "D5: an unknown type confirms by default"
        );
    }

    #[test]
    fn the_label_is_the_family_extension() {
        let registry = builtin();
        assert_eq!(registry.label("material.mat"), Some("mat"));
        assert_eq!(registry.label("Suzanne.glb"), Some("glb"));
        assert_eq!(registry.label("no-extension"), None);
    }

    /// The table names files, not paths: the skin owns the folder (dark and
    /// light type icons live in different directories).
    #[test]
    fn icons_are_file_names_resolved_by_the_skin() {
        let registry = builtin();
        for kind in registry.types().filter(|kind| !kind.icon.is_empty()) {
            assert!(
                !kind.icon.contains('/'),
                "{} icon {} must be a file name, not a path",
                kind.id,
                kind.icon
            );
            assert!(kind.icon.ends_with(".png"), "{} icon", kind.id);
        }
        assert_eq!(registry.icon("Suzanne.png"), Some("texture.png"));
        assert_eq!(
            registry.icon("Suzanne.glb.meta"),
            None,
            "companions draw no icon"
        );
    }

    /// Textures draw their own content; every other family keeps its glyph (and
    /// an unknown file is not a texture just because it might be an image).
    #[test]
    fn only_the_texture_family_draws_its_own_thumbnail() {
        let registry = builtin();
        assert!(registry.thumbnail("Suzanne.png"));
        assert!(registry.thumbnail("UPPER.PNG"));
        assert!(!registry.thumbnail("Suzanne.glb"));
        assert!(!registry.thumbnail("readme.md"));
        assert!(
            !registry.thumbnail("notes.xyz"),
            "an unknown extension has no thumbnail to decode"
        );
        assert!(
            !registry.thumbnail("Suzanne.png.meta"),
            "a sidecar of a texture is a text file, not an image"
        );
    }

    #[test]
    fn the_create_menu_is_folder_then_the_markdown_template() {
        let registry = builtin();
        let entries = registry.create_entries();
        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].label, "Folder");
        assert!(
            entries[0].menu.is_empty(),
            "the folder sits directly under Create"
        );
        assert_eq!(entries[0].target, CreateKind::Folder);
        assert_eq!(entries[0].default_stem, "New Folder");

        assert_eq!(entries[1].menu, ["Document"]);
        assert_eq!(entries[1].label, "Markdown");
        assert_eq!(
            entries[1].target,
            CreateKind::File {
                extension: "md",
                contents: "# "
            }
        );
        assert_eq!(
            registry.lookup("New Document.md").map(|kind| kind.id),
            Some("document"),
            "the created file's row display comes back from the type table"
        );
    }

    #[test]
    fn a_fake_registry_serves_the_same_questions() {
        static FAKE: [FileType; 1] = [FileType {
            id: "fake",
            label: "foo",
            extensions: &["foo"],
            icon: "",
            companion: false,
            thumbnail: false,
            delete_confirm: false,
        }];
        static CREATE: [CreateEntry; 1] = [CreateEntry {
            menu: &["Test"],
            label: "Fake",
            default_stem: "New Fake",
            target: CreateKind::File {
                extension: "foo",
                contents: "",
            },
        }];
        let registry = FileTypeRegistry::new(&FAKE, &CREATE);
        assert_eq!(
            registry.lookup("thing.foo").map(|kind| kind.id),
            Some("fake")
        );
        assert!(
            !registry.delete_confirm("thing.foo"),
            "the type turned confirmation off"
        );
        assert_eq!(registry.create_entries(), CREATE.as_slice());
    }
}
