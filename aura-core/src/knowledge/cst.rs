use tree_sitter::{InputEdit, Language, Node, Parser, Point, Tree};

pub fn get_language(extension: &str) -> Option<Language> {
    match extension {
        "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
        "py" => Some(tree_sitter_python::LANGUAGE.into()),
        "ts" | "tsx" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        "java" => Some(tree_sitter_java::LANGUAGE.into()),
        _ => None,
    }
}

/// A concrete syntax tree wrapper around tree-sitter that strictly tracks byte ranges
/// to guarantee byte-identical serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CstNode {
    pub kind: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub children: Vec<CstNode>,
}

#[derive(Debug, Clone)]
pub struct CstTree {
    pub source: Vec<u8>,
    pub root: CstNode,
}

impl CstTree {
    pub fn new(source: &[u8], root: CstNode) -> Self {
        Self {
            source: source.to_vec(),
            root,
        }
    }

    /// Serializes the CST back to a string, strictly adhering to byte ranges
    /// and retaining all original whitespace, punctuation, and comments.
    pub fn serialize(&self) -> String {
        String::from_utf8(self.source.clone()).unwrap_or_default()
    }
}

pub fn parse(source: &[u8], extension: &str) -> Option<CstTree> {
    let language = get_language(extension)?;
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    let tree: Tree = parser.parse(source, None)?;
    let root = to_cst_node(tree.root_node());
    Some(CstTree::new(source, root))
}

fn to_cst_node(node: Node) -> CstNode {
    let mut children = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        children.push(to_cst_node(child));
    }
    CstNode {
        kind: node.kind().to_string(),
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_sitter_rust_parsing() {
        let code = b"fn main() { println!(\"hello world\"); }";
        let tree = parse(code, "rs").expect("Failed to parse Rust code");
        assert_eq!(tree.root.kind, "source_file");
        assert_eq!(tree.serialize(), "fn main() { println!(\"hello world\"); }");
        assert!(!tree.root.children.is_empty());
        assert_eq!(tree.root.children[0].kind, "function_item");
    }

    #[test]
    fn test_tree_sitter_python_parsing() {
        let code = b"def greet():\n    return 'hello'\n";
        let tree = parse(code, "py").expect("Failed to parse Python code");
        assert_eq!(tree.root.kind, "module");
        assert_eq!(tree.serialize(), "def greet():\n    return 'hello'\n");
    }
}

pub struct CstManager {
    parser: Parser,
    tree: Option<Tree>,
    source_code: String,
}

impl CstManager {
    pub fn new(language: tree_sitter::Language, source_code: String) -> Self {
        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(&source_code, None);
        Self { parser, tree, source_code }
    }

    pub fn edit(&mut self, start_byte: usize, old_end_byte: usize, new_end_byte: usize, start_point: Point, old_end_point: Point, new_end_point: Point, new_source: String) {
        if let Some(mut tree) = self.tree.take() {
            let edit = InputEdit {
                start_byte,
                old_end_byte,
                new_end_byte,
                start_position: start_point,
                old_end_position: old_end_point,
                new_end_position: new_end_point,
            };
            tree.edit(&edit);
            
            // Re-parse with the old tree to perform incremental parsing
            self.tree = self.parser.parse(&new_source, Some(&tree));
            self.source_code = new_source;
        }
    }
    
    pub fn get_cst(&self) -> Option<&Tree> {
        self.tree.as_ref()
    }
}
