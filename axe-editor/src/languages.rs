use tree_sitter::Language;

/// Configuration for a supported programming language.
///
/// Holds the tree-sitter grammar and the highlight query source used
/// to produce syntax highlight spans.
pub struct LanguageConfig {
    /// The tree-sitter grammar for this language.
    pub language: Language,
    /// The highlights.scm query source bundled at compile time.
    pub highlights_query: &'static str,
}

/// Returns the language configuration for a given file extension, or `None`
/// if the extension is not recognised.
pub fn language_for_extension(ext: &str) -> Option<LanguageConfig> {
    let (lang, query) = match ext {
        "rs" => (
            tree_sitter_rust::LANGUAGE.into(),
            include_str!("../queries/rust/highlights.scm"),
        ),
        "py" => (
            tree_sitter_python::LANGUAGE.into(),
            include_str!("../queries/python/highlights.scm"),
        ),
        "js" | "jsx" => (
            tree_sitter_javascript::LANGUAGE.into(),
            include_str!("../queries/javascript/highlights.scm"),
        ),
        "ts" => (
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            include_str!("../queries/typescript/highlights.scm"),
        ),
        "tsx" => (
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            include_str!("../queries/tsx/highlights.scm"),
        ),
        "go" => (
            tree_sitter_go::LANGUAGE.into(),
            include_str!("../queries/go/highlights.scm"),
        ),
        "c" | "h" => (
            tree_sitter_c::LANGUAGE.into(),
            include_str!("../queries/c/highlights.scm"),
        ),
        "cpp" | "cc" | "cxx" | "hpp" => (
            tree_sitter_cpp::LANGUAGE.into(),
            include_str!("../queries/cpp/highlights.scm"),
        ),
        "html" => (
            tree_sitter_html::LANGUAGE.into(),
            include_str!("../queries/html/highlights.scm"),
        ),
        "css" => (
            tree_sitter_css::LANGUAGE.into(),
            include_str!("../queries/css/highlights.scm"),
        ),
        "json" => (
            tree_sitter_json::LANGUAGE.into(),
            include_str!("../queries/json/highlights.scm"),
        ),
        "toml" => (
            tree_sitter_toml_ng::LANGUAGE.into(),
            include_str!("../queries/toml/highlights.scm"),
        ),
        "sh" | "bash" | "zsh" | "fish" => (
            tree_sitter_bash::LANGUAGE.into(),
            include_str!("../queries/bash/highlights.scm"),
        ),
        "md" => (
            tree_sitter_md::LANGUAGE.into(),
            include_str!("../queries/markdown/highlights.scm"),
        ),
        _ => return None,
    };
    Some(LanguageConfig {
        language: lang,
        highlights_query: query,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_extension_returns_config() {
        let config = language_for_extension("rs");
        assert!(config.is_some(), "rs should be a supported extension");
        let config = config.unwrap();
        assert!(!config.highlights_query.is_empty());
    }

    #[test]
    fn python_extension_returns_config() {
        let config = language_for_extension("py");
        assert!(config.is_some());
        assert!(!config.unwrap().highlights_query.is_empty());
    }

    #[test]
    fn javascript_extensions_return_config() {
        for ext in &["js", "jsx"] {
            let config = language_for_extension(ext);
            assert!(config.is_some(), "{ext} should be supported");
        }
    }

    #[test]
    fn typescript_extensions_return_config() {
        for ext in &["ts", "tsx"] {
            let config = language_for_extension(ext);
            assert!(config.is_some(), "{ext} should be supported");
        }
    }

    #[test]
    fn go_extension_returns_config() {
        assert!(language_for_extension("go").is_some());
    }

    #[test]
    fn c_extensions_return_config() {
        for ext in &["c", "h"] {
            assert!(
                language_for_extension(ext).is_some(),
                "{ext} should be supported"
            );
        }
    }

    #[test]
    fn cpp_extensions_return_config() {
        for ext in &["cpp", "cc", "cxx", "hpp"] {
            assert!(
                language_for_extension(ext).is_some(),
                "{ext} should be supported"
            );
        }
    }

    #[test]
    fn web_extensions_return_config() {
        for ext in &["html", "css", "json"] {
            assert!(
                language_for_extension(ext).is_some(),
                "{ext} should be supported"
            );
        }
    }

    #[test]
    fn config_extensions_return_config() {
        for ext in &["toml", "md"] {
            assert!(
                language_for_extension(ext).is_some(),
                "{ext} should be supported"
            );
        }
    }

    #[test]
    fn shell_extensions_return_config() {
        for ext in &["sh", "bash", "zsh", "fish"] {
            assert!(
                language_for_extension(ext).is_some(),
                "{ext} should be supported"
            );
        }
    }

    #[test]
    fn unknown_extension_returns_none() {
        assert!(language_for_extension("xyz").is_none());
        assert!(language_for_extension("").is_none());
        assert!(language_for_extension("docx").is_none());
    }

    #[test]
    fn query_strings_are_valid_tree_sitter_queries() {
        // Verify that all bundled queries parse without errors.
        let extensions = [
            "rs", "py", "js", "ts", "tsx", "go", "c", "cpp", "html", "css", "json", "toml", "sh",
            "md",
        ];
        for ext in extensions {
            let config =
                language_for_extension(ext).unwrap_or_else(|| panic!("expected config for {ext}"));
            let result = tree_sitter::Query::new(&config.language, config.highlights_query);
            assert!(
                result.is_ok(),
                "query for {ext} failed to parse: {:?}",
                result.err()
            );
        }
    }
}
