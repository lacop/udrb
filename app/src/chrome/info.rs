// TODO: Extract this into separate crate and make it fancier and better tested.

use markup5ever_rcdom as rcdom;
use std::collections::HashMap;

#[derive(Debug)]
pub struct PageInfo {
    pub icon: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug, Hash, PartialEq, Eq)]
enum Node {
    Meta {
        name: Option<String>,
        property: Option<String>,
    },
    Link {
        rel: Option<String>,
        ty: Option<String>,
    },
}

impl PageInfo {
    pub fn from_html(html: &str) -> anyhow::Result<PageInfo> {
        use html5ever::tendril::TendrilSink;

        let mut reader = std::io::Cursor::new(html);
        let document = html5ever::parse_document(rcdom::RcDom::default(), Default::default())
            .from_utf8()
            .read_from(&mut reader)?;
        let mut values = HashMap::new();
        Self::walk(&mut values, &document.document);

        //println!("{:#?}", values);

        Ok(PageInfo {
            icon: Self::get_first_value(
                &values,
                &[
                    Node::Link {
                        rel: Some("icon".to_string()),
                        ty: None,
                    },
                    Node::Link {
                        rel: Some("icon".to_string()),
                        ty: Some("image/png".to_string()),
                    },
                    Node::Link {
                        rel: Some("shortcut icon".to_string()),
                        ty: Some("image/x-icon".to_string()),
                    },
                ],
            ),
            description: Self::get_first_value(
                &values,
                &[
                    Node::Meta {
                        name: Some("description".to_string()),
                        property: None,
                    },
                    Node::Meta {
                        name: None,
                        property: Some("og:description".to_string()),
                    },
                ],
            ),
            image: Self::get_first_value(
                &values,
                &[Node::Meta {
                    name: None,
                    property: Some("og:image".to_string()),
                }],
            ),
            author: Self::get_first_value(
                &values,
                &[Node::Meta {
                    name: Some("author".to_string()),
                    property: None,
                }],
            )
            // Some sites include url or something else in the author field.
            // Presence of dot is a crude heuristic to filter out such cases.
            .filter(|a| !a.contains(".")),
        })
    }

    fn get_first_value(values: &HashMap<Node, String>, keys: &[Node]) -> Option<String> {
        for key in keys {
            if let Some(value) = values.get(key) {
                return Some(value.clone());
            }
        }
        None
    }

    fn get_attribute(name: &str, attrs: &[html5ever::interface::Attribute]) -> Option<String> {
        for attr in attrs {
            if attr.name.local.to_string() == name {
                return Some(attr.value.to_string());
            }
        }
        None
    }

    fn walk(values: &mut HashMap<Node, String>, handle: &rcdom::Handle) {
        if let rcdom::NodeData::Element {
            ref name,
            ref attrs,
            ..
        } = handle.data
        {
            let attrs = attrs.borrow();
            match name.local.to_string().as_str() {
                "meta" => {
                    let content = Self::get_attribute("content", &attrs);
                    if let Some(value) = content {
                        values.insert(
                            Node::Meta {
                                name: Self::get_attribute("name", &attrs),
                                property: Self::get_attribute("property", &attrs),
                            },
                            value,
                        );
                    }
                }
                "link" => {
                    let content = Self::get_attribute("href", &attrs);
                    if let Some(value) = content {
                        values.insert(
                            Node::Link {
                                rel: Self::get_attribute("rel", &attrs),
                                ty: Self::get_attribute("type", &attrs),
                            },
                            value,
                        );
                    }
                }
                _ => {}
            }
        }
        for child in handle.children.borrow().iter() {
            Self::walk(values, child);
        }
    }
}
