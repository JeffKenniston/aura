
use tree_sitter::Language;
use std::path::Path;
use streaming_iterator::StreamingIterator;

pub struct ParserRegistry;

impl ParserRegistry {
    pub fn get_language_for_file(path: &Path) -> Option<Language> {
        let ext = path.extension()?.to_str()?;
        match ext {
            "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
            "py" => Some(tree_sitter_python::LANGUAGE.into()),
            "js" => Some(tree_sitter_javascript::LANGUAGE.into()),
            "ts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            "go" => Some(tree_sitter_go::LANGUAGE.into()),
            "c" => Some(tree_sitter_c::LANGUAGE.into()),
            "cpp" | "cc" | "cxx" => Some(tree_sitter_cpp::LANGUAGE.into()),
            "java" => Some(tree_sitter_java::LANGUAGE.into()),
            "cs" => Some(tree_sitter_c_sharp::LANGUAGE.into()),
            "rb" => Some(tree_sitter_ruby::LANGUAGE.into()),
            _ => None,
        }
    }

    pub fn extract_symbols(kg: &crate::knowledge::KnowledgeGraph, path: &Path, content: &[u8], tree: &tree_sitter::Tree) {
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        
        let query_str = match ext {
            "rs" => "(function_item name: (identifier) @name) (struct_item name: (type_identifier) @name)",
            "py" => "(function_definition name: (identifier) @name) (class_definition name: (identifier) @name)",
            "js" => "(function_declaration name: (identifier) @name) (class_declaration name: (identifier) @name)",
            "ts" => "(function_declaration name: (_) @name) (class_declaration name: (_) @name)",
            "go" => "(function_declaration name: (identifier) @name) (type_spec name: (type_identifier) @name)",
            "java" => "(method_declaration name: (identifier) @name) (class_declaration name: (identifier) @name)",
            "c" => "(function_definition declarator: (function_declarator declarator: (identifier) @name))",
            "cpp" | "cc" | "cxx" => "(function_definition declarator: (function_declarator declarator: (identifier) @name)) (class_specifier name: (type_identifier) @name)",
            "cs" => "(method_declaration name: (identifier) @name) (class_declaration name: (identifier) @name)",
            "rb" => "(method name: (identifier) @name) (class name: (constant) @name)",
            _ => return,
        };

        if let Some(language) = Self::get_language_for_file(path) {
            if let Ok(query) = tree_sitter::Query::new(&language, query_str) {
                let mut cursor = tree_sitter::QueryCursor::new();
                let mut matches = cursor.matches(&query, tree.root_node(), content);
                while let Some(m) = matches.next() {
                    for cap in m.captures {
                        let node = cap.node;
                        if let Ok(name) = std::str::from_utf8(&content[node.start_byte()..node.end_byte()]) {
                            let file_path = path.to_string_lossy().to_string();
                            let _ = kg.insert_local_definition(&file_path, name, "symbol", node.start_byte(), node.end_byte());
                        }
                    }
                }
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::KnowledgeGraph;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_universal_tags_extraction() {
        let kg = KnowledgeGraph::new(":memory:").unwrap();
        let dir = tempdir().unwrap();

        let files = vec![
            ("main.rs", "fn rust_func() {} struct RustStruct {}"),
            ("main.py", "def python_func():\n    pass\nclass PythonClass:\n    pass"),
            ("main.js", "function js_func() {} class JsClass {}"),
            ("main.ts", "function ts_func() {} class TsClass {}"),
            ("main.go", "func go_func() {} type GoStruct struct {}"),
            ("main.java", "class JavaClass { void java_func() {} }"),
            ("main.c", "void c_func() {}"),
            ("main.cpp", "void cpp_func() {} class CppClass {};"),
            ("main.cs", "class CsClass { void cs_func() {} }"),
            ("main.rb", "class RbClass\n  def rb_func\n  end\nend"),
        ];

        for (name, content) in files {
            let path = dir.path().join(name);
            fs::write(&path, content).unwrap();
            
            if let Some(language) = ParserRegistry::get_language_for_file(&path) {
                let mut parser = tree_sitter::Parser::new();
                parser.set_language(&language).unwrap();
                let tree = parser.parse(content.as_bytes(), None).unwrap();
                
                ParserRegistry::extract_symbols(&kg, &path, content.as_bytes(), &tree);
            } else {
                panic!("Language not found for {}", name);
            }
        }

        // Verify the extracted symbols are in the KG
        // We'll just check if we got roughly the expected number of definitions.
        // There are 19 symbols across the 10 files.
        let mut stmt = kg.conn.prepare("SELECT symbol_name FROM local_definitions").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut count = 0;
        while let Some(row) = rows.next().unwrap() {
            let name: String = row.get(0).unwrap();
            println!("Found symbol: {}", name);
            count += 1;
        }

        assert!(count >= 19, "Expected at least 19 symbols, found {}", count);
    }
}
