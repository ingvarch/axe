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

/// Comment token configuration for a language.
///
/// Populated independently of `LanguageConfig` because comment-toggling
/// should work for languages without a bundled tree-sitter grammar
/// (e.g. SQL, Lua, YAML) and should not require loading the grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentTokens {
    /// Line-comment prefix (e.g. `"//"` for Rust, `"#"` for Python).
    pub line: Option<&'static str>,
    /// Block-comment open/close pair (e.g. `("/*", "*/")` for C-family).
    pub block: Option<(&'static str, &'static str)>,
}

/// Returns the comment token configuration for a given file extension,
/// or `None` if comments are unknown for that extension.
pub fn comment_tokens_for_extension(ext: &str) -> Option<CommentTokens> {
    let tokens = match ext {
        // C-family: line + block.
        "rs" | "js" | "jsx" | "ts" | "tsx" | "go" | "c" | "h" | "cpp" | "cc" | "cxx" | "hpp"
        | "java" | "swift" | "kt" | "kts" | "cs" | "scala" | "dart" | "css" | "scss" | "less"
        | "tf" | "tfvars" | "hcl" => CommentTokens {
            line: Some("//"),
            block: Some(("/*", "*/")),
        },
        // Hash-comment languages.
        "py" | "sh" | "bash" | "zsh" | "fish" | "toml" | "yaml" | "yml" | "rb" | "pl" | "r"
        | "make" | "mk" | "dockerfile" => CommentTokens {
            line: Some("#"),
            block: None,
        },
        // SQL-style double-dash.
        "sql" | "lua" | "hs" | "elm" => CommentTokens {
            line: Some("--"),
            block: None,
        },
        // Markup block-only comments.
        "html" | "xml" | "md" | "svg" => CommentTokens {
            line: None,
            block: Some(("<!--", "-->")),
        },
        // JSON has no official comments; skip.
        _ => return None,
    };
    Some(tokens)
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
        "tf" | "tfvars" | "hcl" => (
            tree_sitter_hcl::LANGUAGE.into(),
            include_str!("../queries/hcl/highlights.scm"),
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
    fn terraform_extensions_return_config() {
        for ext in &["tf", "tfvars", "hcl"] {
            let config = language_for_extension(ext);
            assert!(config.is_some(), "{ext} should be supported");
            assert!(!config.unwrap().highlights_query.is_empty());
        }
    }

    #[test]
    fn unknown_extension_returns_none() {
        assert!(language_for_extension("xyz").is_none());
        assert!(language_for_extension("").is_none());
        assert!(language_for_extension("docx").is_none());
    }

    #[test]
    fn comment_tokens_rust_has_line_and_block() {
        let t = comment_tokens_for_extension("rs").unwrap();
        assert_eq!(t.line, Some("//"));
        assert_eq!(t.block, Some(("/*", "*/")));
    }

    #[test]
    fn comment_tokens_python_has_hash_no_block() {
        let t = comment_tokens_for_extension("py").unwrap();
        assert_eq!(t.line, Some("#"));
        assert!(t.block.is_none());
    }

    #[test]
    fn comment_tokens_html_has_block_no_line() {
        let t = comment_tokens_for_extension("html").unwrap();
        assert!(t.line.is_none());
        assert_eq!(t.block, Some(("<!--", "-->")));
    }

    #[test]
    fn comment_tokens_sql_has_double_dash() {
        let t = comment_tokens_for_extension("sql").unwrap();
        assert_eq!(t.line, Some("--"));
    }

    #[test]
    fn comment_tokens_unknown_extension_is_none() {
        assert!(comment_tokens_for_extension("").is_none());
        assert!(comment_tokens_for_extension("xyz").is_none());
        // JSON intentionally has no comment tokens.
        assert!(comment_tokens_for_extension("json").is_none());
    }

    #[test]
    fn comment_tokens_all_c_family_match() {
        for ext in &["rs", "js", "ts", "go", "c", "cpp", "java", "swift", "css"] {
            let t = comment_tokens_for_extension(ext)
                .unwrap_or_else(|| panic!("expected tokens for {ext}"));
            assert_eq!(t.line, Some("//"), "{ext} should use // line comment");
        }
    }

    #[test]
    fn query_strings_are_valid_tree_sitter_queries() {
        // Verify that all bundled queries parse without errors.
        let extensions = [
            "rs", "py", "js", "ts", "tsx", "go", "c", "cpp", "html", "css", "json", "toml", "sh",
            "md", "tf", "tfvars", "hcl",
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
