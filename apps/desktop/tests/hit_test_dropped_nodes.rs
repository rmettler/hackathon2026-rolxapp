//! A pointer event can arrive between a DOM change and the next layout, when the paint lists still
//! name nodes the change dropped. Stock `blitz-dom` panics then, see `vendor/README.md`.

use blitz_dom::{Attribute, BaseDocument, DocumentConfig, NodeId, QualName, ns, qual_name};
use blitz_traits::shell::{ColorScheme, Viewport};

fn element(document: &mut BaseDocument, parent: NodeId, tag: &str, style: &str) -> NodeId {
    let mut mutator = document.mutate();
    let attrs = vec![Attribute {
        name: qual_name!("style", html),
        value: style.to_owned(),
    }];
    let id = mutator.create_element(QualName::new(None, ns!(html), tag.into()), attrs);
    mutator.append_children(parent, &[id]);
    id
}

fn hit(document: &BaseDocument, x: f32, y: f32) -> Option<NodeId> {
    document.hit(x, y).map(|hit| hit.node_id)
}

#[test]
fn hit_test_skips_nodes_dropped_since_the_last_resolve() {
    let mut document = BaseDocument::new(DocumentConfig {
        viewport: Some(Viewport::new(400, 300, 1.0, ColorScheme::Light)),
        ..Default::default()
    });
    let root = document.root_node().id;
    let html = element(&mut document, root, "html", "");
    let body = element(&mut document, html, "body", "margin:0;height:300px");
    let content = element(
        &mut document,
        body,
        "div",
        "position:relative;z-index:0;width:300px;height:300px",
    );
    let negative_z = element(
        &mut document,
        content,
        "div",
        "position:relative;left:320px;width:50px;height:50px;z-index:-1",
    );
    let in_flow = element(&mut document, content, "div", "width:100px;height:100px");
    let paragraph = element(
        &mut document,
        content,
        "p",
        "margin:0;font-size:20px;line-height:1",
    );
    let span = element(&mut document, paragraph, "span", "");
    {
        let mut mutator = document.mutate();
        let text = mutator.create_text_node("some text");
        mutator.append_children(span, &[text]);
    }
    let positive_z = element(
        &mut document,
        body,
        "div",
        "position:fixed;left:0;bottom:0;width:200px;height:20px;z-index:10",
    );
    document.resolve(0.0);

    assert_eq!(hit(&document, 10.0, 290.0), Some(positive_z));
    assert_eq!(hit(&document, 330.0, 10.0), Some(negative_z));
    assert_eq!(hit(&document, 10.0, 60.0), Some(in_flow));
    assert_eq!(hit(&document, 10.0, 160.0), Some(span));

    {
        let mut mutator = document.mutate();
        for node in [positive_z, negative_z, in_flow, span] {
            mutator.remove_and_drop_node(node);
        }
    }

    assert_eq!(hit(&document, 10.0, 290.0), Some(content));
    assert_eq!(hit(&document, 330.0, 10.0), Some(body));
    assert_eq!(hit(&document, 10.0, 60.0), Some(content));
    assert_eq!(hit(&document, 10.0, 160.0), Some(paragraph));
}
