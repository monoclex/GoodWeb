use std::collections::HashMap;
use roxmltree::{Attribute, Children, Document, Node, NodeType};
use xmlwriter::{Indent, Options, XmlWriter};
use crate::templating::TemplateEngine;

#[derive(Debug)]
pub struct Component {
    xml_data: String,
    css_data: String,
    // document: Document,
}

impl Component {
    pub fn document(&self) -> Document<'_> {
        Document::parse(self.xml_data.as_ref()).unwrap()
    }
}

impl std::default::Default for Component {
    fn default() -> Component {
        Component {
            xml_data: String::new(),
            css_data: String::new(),
        }
    }
}

// #[derive(Clone, Copy)]
pub struct ComponentStore {
    components: HashMap<String, Component>
}

impl ComponentStore {
    pub fn new() -> ComponentStore {
        ComponentStore {
            components: HashMap::new(),
        }
    }

    #[inline]
    pub fn find_component(&self, name: &str) -> Option<&Component> {
        self.components.get(name)
    }

    pub fn store_xml(&mut self, name: String, data: String) -> Result<(), ()> {
        let entry = self.components.entry(name);

        // ensures that component.document() doesn't fail
        let document = match Document::parse(data.as_ref()) {
            Ok(document) => document,
            Err(_) => return Result::Err(()),
        };

        let mut component = entry.or_default();

        component.xml_data = data;

        Ok(())
    }

    #[inline]
    pub fn store_css(&mut self, name: String, data: String) -> Result<(), ()> {
        let entry = self.components.entry(name);
        let mut component = entry.or_default();

        component.css_data = data;

        Ok(())
    }
}

// TODO: use Result to detail errors
pub fn build_page(page: &Component, components: &ComponentStore) -> Option<String> {
    let handlebars = handlebars::Handlebars::new();
    let engine = TemplateEngine::new(&handlebars);

    // no idea how much we'll need, but let's allocate a pretty large buffer just in case
    let writer = XmlWriter::with_capacity(1_000, Options {
        use_single_quote: false,
        indent: Indent::None,
        attributes_indent: Indent::None,
        ..Options::default()
    });

    // we pass in the state and let it own everything, and hope we get the String back
    let writer = compute_recursive_pre(
        writer,
        components,
        page.document().root(),
        &engine,

        None,
        None,
    )?;

    Some(writer.end_document())
}

#[inline]
fn compute_recursive_pre(
    writer: XmlWriter,
    components: &ComponentStore,
    node: Node<'_, '_>,
    engine: &TemplateEngine<'_, '_>,

    component_attributes: Option<&[Attribute<'_>]>,
    goodweb_inner: Option<Node<'_, '_>>,
) -> Option<XmlWriter> {
    let engine = match component_attributes {
        Some(attributes) => {
            println!("computing cmp attribs: {:?}", attributes);
            engine.compute_state(attributes)?
        },
        None => engine.compute_state(&[])?,
    };

    compute_recursive(
        writer,
        components,
        node.children(),
        &engine,
        goodweb_inner,
    )
}

fn compute_recursive<'a, 'b>(
    writer: XmlWriter,
    components: &ComponentStore,
    children: Children<'_, '_>,
    engine: &TemplateEngine<'_, '_>,
    
    goodweb_inner: Option<Node<'_, '_>>,
) -> Option<XmlWriter> {
    let mut writer = writer;

    for child in children {
        println!("deal: {:?}", child);
        match child.node_type() {
            NodeType::Root => panic!("Should never be on a root node."),
            NodeType::Comment => continue,
            NodeType::PI => continue,
            NodeType::Text => {
                writer.write_text(engine.solve(child.text()?)?.trim());
                continue;
            },
            NodeType::Element => {
                let name = child.tag_name().name();

                if is_goodweb_component(&child) {
                    // TODO: support these
                    match get_goodweb_component(name) {
                        GoodWebComponent::Inner => {
                            let goodweb_inner = match goodweb_inner {
                                Some(goodweb_inner) => goodweb_inner,
                                None => {
                                    println!("[WARN] invalid GoodWeb-Inner declaration");
                                    continue;
                                }
                            };

                            writer = compute_recursive_pre(
                                writer,
                                components,
                                goodweb_inner,
                                engine,

                                None,
                                None,
                            )?;
                        },
                        GoodWebComponent::Styles => continue,
                        GoodWebComponent::None => {
                            println!("Invalid GoodWeb component '{}' - Expected 'Inner' or 'Styles'.", name);
                            continue;
                        }
                    }
                }

                if !is_first_char_uppercase(name)? {
                    // if the first character is not uppercase, we treat it as some html element.
                    //
                    // html elements:
                    // - attributes are not computed into the template engine
                    // - body is analyzed

                    writer.start_element(name);

                    for attribute in child.attributes() {
                        writer.write_attribute(attribute.name(), &engine.solve(attribute.value())?);
                    }

                    writer = compute_recursive_pre(
                        writer,
                        components,
                        child,
                        engine,
                        
                        None,
                        goodweb_inner
                    )?;

                    writer.end_element();
                }
                else {
                    // we're dealing with a component now.
                    // all we need to do is compute the component with its state.
                    // we don't need to write anything, we'll leave all the writing to
                    // the component's components.

                    let component = match components.find_component(name) {
                        Some(component) => component,
                        None => {
                            println!("[WARN] no component found for element '{}'", name);
                            continue;
                        }
                    };

                    println!("cmp: innr");
                    writer = compute_recursive_pre(
                        writer,
                        components,
                        component.document().root(),
                        engine,

                        Some(child.attributes()),
                        // the <GoodWeb:Inner> will be determined by the children
                        // of the compnent
                        Some(child)
                    )?;
                }
            },
        };
    }

    Some(writer)
}

#[inline]
fn is_first_char_uppercase(slice: &str) -> Option<bool> {
    let character = slice.chars().next()?;
    Some(character >= 'A' && character <= 'Z')
}

enum GoodWebComponent {
    None,
    Inner,
    Styles,
}

#[inline]
fn is_goodweb_component<'a, 'b, 'c>(node: &'a roxmltree::Node<'b, 'c>) -> bool {
    node.tag_name().name().starts_with("GoodWeb-")
}

#[inline]
fn get_goodweb_component(name: &str) -> GoodWebComponent {
    match name {
        "GoodWeb-Inner" => GoodWebComponent::Inner,
        "GoodWeb-Styles" => GoodWebComponent::Styles,
        _ => GoodWebComponent::None
    }
}