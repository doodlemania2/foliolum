//! What the ribbon holds: tabs, groups and buttons, in our own plain words.
//!
//! Clean room: these names and groupings are ours. They are not taken from Microsoft Publisher's
//! ribbon, menus or help, and nobody wrote them from its screenshots. Every button runs something
//! DesignCraft already has (a menu command, a tool or a panel), so the ribbon adds no behaviour.

/// What a button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// A command from DesignCraft's menus, run as a menu click (so it opens its dialog if it has
    /// one): the id without its `cmd:`/`ui:` prefix, and its parameters as JSON ("" for none).
    Menu(&'static str, &'static str),
    /// Pick one of DesignCraft's tools (`designcraft_tools::TOOL_GROUPS` ids).
    Tool(&'static str),
}

/// One button: its label, what it does, and one of DesignCraft's icon names if one fits.
#[derive(Clone, Copy, Debug)]
pub struct Button {
    pub label: &'static str,
    pub action: Action,
    pub icon: Option<&'static str>,
}

/// A captioned group of buttons.
#[derive(Clone, Copy, Debug)]
pub struct Group {
    pub caption: &'static str,
    pub buttons: &'static [Button],
}

/// A ribbon tab.
#[derive(Clone, Copy, Debug)]
pub struct Tab {
    pub name: &'static str,
    pub groups: &'static [Group],
}

const fn menu(label: &'static str, id: &'static str, icon: Option<&'static str>) -> Button {
    Button { label, action: Action::Menu(id, ""), icon }
}

const fn menu_with(label: &'static str, id: &'static str, params: &'static str, icon: Option<&'static str>) -> Button {
    Button { label, action: Action::Menu(id, params), icon }
}

const fn tool(label: &'static str, id: &'static str, icon: &'static str) -> Button {
    Button { label, action: Action::Tool(id), icon: Some(icon) }
}

/// The ribbon, left to right. The tab shown first is [`START_TAB`].
pub const TABS: &[Tab] = &[
    Tab {
        name: "File",
        groups: &[
            Group {
                caption: "Document",
                buttons: &[
                    menu("New…", "app.newDocumentDialog", None),
                    menu("Open…", "app.openDialog", None),
                    menu("Save", "app.save", None),
                    menu("Save a copy…", "app.saveCopyDialog", None),
                ],
            },
            Group {
                caption: "Print and share",
                buttons: &[
                    menu("PDF for printing…", "app.exportPdf", None),
                    menu("Picture…", "app.exportPng", None),
                    menu("Booklet…", "app.printBooklet", None),
                ],
            },
            Group {
                caption: "Check",
                buttons: &[menu_with("Before you print", "window.panel", r#"{"panel": "preflight"}"#, Some("panel-preflight"))],
            },
        ],
    },
    Tab {
        name: "Home",
        groups: &[
            Group { caption: "Undo", buttons: &[menu("Undo", "edit.undo", None), menu("Redo", "edit.redo", None)] },
            Group {
                caption: "Clipboard",
                buttons: &[menu("Cut", "edit.cut", None), menu("Copy", "edit.copy", None), menu("Paste", "edit.paste", None)],
            },
            Group {
                caption: "Pointer",
                buttons: &[
                    tool("Select", "selection", "tool-selection"),
                    tool("Move the view", "hand", "tool-hand"),
                    tool("Zoom", "zoom", "tool-zoom"),
                ],
            },
            Group {
                caption: "Words",
                buttons: &[menu("Find and replace…", "app.findChange", Some("search")), menu("Spelling…", "spelling.check", None)],
            },
        ],
    },
    Tab {
        name: "Insert",
        groups: &[
            Group {
                caption: "Text and pictures",
                buttons: &[tool("Text box", "type", "tool-type"), menu("Picture…", "app.placeDialog", Some("tool-rect-frame"))],
            },
            Group {
                caption: "Shapes",
                buttons: &[tool("Rectangle", "rectangle", "tool-rect"), tool("Oval", "ellipse", "tool-ellipse"), tool("Line", "line", "tool-line")],
            },
            Group { caption: "Pages", buttons: &[menu("Page", "layout.pages.insert", Some("plus"))] },
        ],
    },
    Tab {
        name: "Page",
        groups: &[
            Group {
                caption: "Setup",
                buttons: &[
                    menu("Page size…", "layout.documentSetup", Some("orient-portrait")),
                    menu("Margins and columns…", "layout.marginsAndColumns", Some("margin-top")),
                ],
            },
            Group {
                caption: "Pages",
                buttons: &[menu("Add a page", "layout.pages.insert", Some("plus")), menu("Delete this page", "app.deletePage", Some("trash"))],
            },
            Group {
                caption: "Go to",
                buttons: &[
                    menu("First", "view.firstPage", Some("first")),
                    menu("Previous", "view.previousPage", Some("prev")),
                    menu("Next", "view.nextPage", Some("next")),
                    menu("Last", "view.lastPage", Some("last")),
                ],
            },
        ],
    },
];

/// The tab shown when Foliolum starts.
pub const START_TAB: usize = 1;

/// Every button on every tab.
pub fn buttons() -> impl Iterator<Item = &'static Button> {
    TABS.iter().flat_map(|t| t.groups.iter()).flat_map(|g| g.buttons.iter())
}
