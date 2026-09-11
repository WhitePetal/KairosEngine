//! Tests for [`AssetPath`] and [`AssetSourceId`].

use std::path::{Path, PathBuf};

use crate::io::AssetSourceId;
use crate::path::{AssetPath, ParseAssetPathError};

#[test]
fn parse_reads_source_path_and_label() {
    assert_eq!(AssetPath::parse("a/b.test"), AssetPath::from("a/b.test"));

    let path = AssetPath::parse("custom://a/b.test#Foo");
    assert_eq!(path.source(), &AssetSourceId::Name("custom".into()));
    assert_eq!(path.path(), Path::new("a/b.test"));
    assert_eq!(path.label(), Some("Foo"));
}

#[test]
fn parse_rejects_malformed_paths() {
    assert_eq!(
        AssetPath::try_parse("://x"),
        Err(ParseAssetPathError::MissingSource)
    );
    assert_eq!(
        AssetPath::try_parse("a/b.test#"),
        Err(ParseAssetPathError::MissingLabel)
    );
    assert_eq!(
        AssetPath::try_parse("#insource://a/b.test"),
        Err(ParseAssetPathError::InvalidSourceSyntax)
    );
    assert_eq!(
        AssetPath::try_parse("source://a/b.test#://inlabel"),
        Err(ParseAssetPathError::InvalidLabelSyntax)
    );
}

#[test]
fn display_round_trips_the_parts() {
    assert_eq!(AssetPath::parse("a/b.test").to_string(), "a/b.test");
    assert_eq!(
        AssetPath::parse("custom://a/b.test#Foo").to_string(),
        "custom://a/b.test#Foo"
    );
}

#[test]
fn without_label_and_parent_drop_the_label() {
    let path = AssetPath::parse("custom://a/b.test#Foo");
    assert_eq!(path.without_label().label(), None);
    assert_eq!(path.without_label().path(), Path::new("a/b.test"));
    assert_eq!(path.without_label().source(), path.source());

    assert_eq!(
        AssetPath::from("a/b.test").parent(),
        Some(AssetPath::from("a"))
    );
    assert_eq!(
        AssetPath::from("a/b.test#Foo").parent(),
        Some(AssetPath::from("a"))
    );
    assert_eq!(AssetPath::from("a").parent(), Some(AssetPath::from("")));
    assert_eq!(AssetPath::from("").parent(), None);
}

#[test]
fn with_label_and_with_source_replace_existing_parts() {
    let path = AssetPath::parse("a/b.test#Foo");
    assert_eq!(path.clone().with_label("Bar"), AssetPath::parse("a/b.test#Bar"));
    assert_eq!(
        path.with_source("ftp"),
        AssetPath::parse("ftp://a/b.test#Foo")
    );
}

#[test]
fn is_unapproved_detects_escapes() {
    assert!(!AssetPath::parse("thingy.png").is_unapproved());
    assert!(!AssetPath::parse("gui/thingy.png").is_unapproved());
    assert!(!AssetPath::parse("embedded://thingy.png").is_unapproved());
    assert!(AssetPath::parse("../thingy.png").is_unapproved());
    assert!(AssetPath::parse("folder/../../thingy.png").is_unapproved());
    assert!(AssetPath::parse("/home/thingy.png").is_unapproved());
}

#[test]
fn extensions_handle_multiple_dots_and_queries() {
    let path = AssetPath::parse("my_asset.config.ron");
    assert_eq!(path.get_full_extension(), Some("config.ron"));
    assert_eq!(path.get_extension(), Some("ron"));

    let path = AssetPath::parse("data.bin?version=2");
    assert_eq!(path.get_full_extension(), Some("bin"));
    assert_eq!(path.get_extension(), Some("bin"));

    assert_eq!(AssetPath::parse("no_extension").get_extension(), None);
}

#[test]
fn resolve_concatenates_and_normalizes() {
    let base = AssetPath::parse("a/b");
    assert_eq!(base.resolve(&AssetPath::parse("c")), AssetPath::parse("a/b/c"));
    assert_eq!(base.resolve(&AssetPath::parse("./c")), AssetPath::parse("a/b/c"));
    assert_eq!(base.resolve(&AssetPath::parse("../c")), AssetPath::parse("a/c"));
    assert_eq!(base.resolve(&AssetPath::parse("/c")), AssetPath::parse("c"));
    assert_eq!(
        AssetPath::parse("a/b.png").resolve(&AssetPath::parse("#c")),
        AssetPath::parse("a/b.png#c")
    );
    assert_eq!(
        AssetPath::parse("a/b.png#c").resolve(&AssetPath::parse("#d")),
        AssetPath::parse("a/b.png#d")
    );
}

#[test]
fn into_owned_is_static_and_equivalent() {
    let owned = AssetPath::parse("a/b.test#Foo").into_owned();
    let static_path: AssetPath<'static> = owned.clone();
    assert_eq!(static_path, owned);
    assert_eq!(owned.source().clone_owned(), AssetSourceId::Default);
}

#[test]
fn ron_round_trips_through_the_string_form() {
    let path = AssetPath::parse("custom://a/b.test#Foo");
    let serialized = ron::to_string(&path).unwrap();
    assert_eq!(serialized, "\"custom://a/b.test#Foo\"");

    let parsed: AssetPath<'static> = ron::from_str(&serialized).unwrap();
    assert_eq!(parsed, path);

    assert!(ron::from_str::<AssetPath<'static>>("\"a/b.test#\"").is_err());
}

#[test]
fn from_path_buf_uses_the_default_source() {
    let path = AssetPath::from_path_buf(PathBuf::from("res/models/Suzanne.mesh"));
    assert_eq!(path.source(), &AssetSourceId::Default);
    assert_eq!(path.label(), None);
    assert_eq!(path.path(), Path::new("res/models/Suzanne.mesh"));
}

#[test]
fn asset_source_id_reports_its_name() {
    assert_eq!(AssetSourceId::Default.as_str(), None);
    assert_eq!(AssetSourceId::Name("remote".into()).as_str(), Some("remote"));
    assert_eq!(
        AssetSourceId::from(Some("remote")).as_str(),
        Some("remote")
    );
    assert_eq!(AssetSourceId::from(None::<&'static str>), AssetSourceId::Default);
}
