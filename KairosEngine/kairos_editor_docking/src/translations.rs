

#[derive(Clone)]
pub struct TabContextMenuTranslations {
    pub close_button: String,
    pub eject_button: String,
}

impl TabContextMenuTranslations {
    pub fn english() -> Self {
        Self {
            close_button: String::from("Close"),
            eject_button: String::from("Eject")
        }
    }
}

#[derive(Clone)]
pub struct LeafTranslations {

}

impl LeafTranslations {
    pub fn english() -> Self {
        Self {  }
    }
}

#[derive(Clone)]
pub struct Translations {
    pub tab_context_menu: TabContextMenuTranslations,
    pub leaf: LeafTranslations,
}

impl Translations {
    pub fn english() -> Self {
        Self {
            tab_context_menu: TabContextMenuTranslations::english(),
            leaf: LeafTranslations::english()
        }
    }
}
