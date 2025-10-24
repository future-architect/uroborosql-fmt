use postgresql_cst_parser::tree_sitter::Node;

/// Collect all nodes in preorder traversal.
pub fn collect_preorder<'tree>(root: Node<'tree>) -> Vec<Node<'tree>> {
    let mut nodes = Vec::new();
    walk_preorder(root, &mut nodes, |vec, node| vec.push(node));
    nodes
}

fn walk_preorder<'tree, T>(
    root: Node<'tree>,
    state: &mut T,
    mut visit: impl FnMut(&mut T, Node<'tree>),
) {
    let mut cursor = root.walk();

    loop {
        visit(state, cursor.node());

        if cursor.goto_first_child() {
            continue;
        }

        loop {
            if cursor.goto_next_sibling() {
                break;
            }

            if !cursor.goto_parent() {
                return;
            }
        }
    }
}
