use std::path::Path;

/// Maps a file path to its LSP language identifier based on file extension.
///
/// Returns `None` for unknown extensions or files without extensions.
pub fn language_id_for_path(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?;
    match ext {
        "rs" => Some("rust"),
        "py" => Some("python"),
        "js" | "jsx" => Some("javascript"),
        "ts" | "tsx" => Some("typescript"),
        "go" => Some("go"),
        "c" => Some("c"),
        "cpp" | "cc" | "cxx" => Some("cpp"),
        "h" | "hpp" => Some("c"),
        "toml" => Some("toml"),
        "yaml" | "yml" => Some("yaml"),
        "json" => Some("json"),
        "md" => Some("markdown"),
        "html" => Some("html"),
        "css" => Some("css"),
        "sh" | "bash" | "zsh" => Some("shellscript"),
        "tf" | "tfvars" | "hcl" => Some("terraform"),
        "lua" => Some("lua"),
        "java" => Some("java"),
        "rb" => Some("ruby"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn rust_extension() {
        assert_eq!(language_id_for_path(Path::new("main.rs")), Some("rust"));
    }

    #[test]
    fn python_extension() {
        assert_eq!(language_id_for_path(Path::new("script.py")), Some("python"));
    }

    #[test]
    fn javascript_extensions() {
        assert_eq!(
            language_id_for_path(Path::new("app.js")),
            Some("javascript")
        );
        assert_eq!(
            language_id_for_path(Path::new("component.jsx")),
            Some("javascript")
        );
    }

    #[test]
    fn typescript_extensions() {
        assert_eq!(
            language_id_for_path(Path::new("app.ts")),
            Some("typescript")
        );
        assert_eq!(
            language_id_for_path(Path::new("component.tsx")),
            Some("typescript")
        );
    }

    #[test]
    fn go_extension() {
        assert_eq!(language_id_for_path(Path::new("main.go")), Some("go"));
    }

    #[test]
    fn c_and_cpp_extensions() {
        assert_eq!(language_id_for_path(Path::new("main.c")), Some("c"));
        assert_eq!(language_id_for_path(Path::new("main.cpp")), Some("cpp"));
        assert_eq!(language_id_for_path(Path::new("main.cc")), Some("cpp"));
        assert_eq!(language_id_for_path(Path::new("main.cxx")), Some("cpp"));
        assert_eq!(language_id_for_path(Path::new("header.h")), Some("c"));
        assert_eq!(language_id_for_path(Path::new("header.hpp")), Some("c"));
    }

    #[test]
    fn shell_extensions() {
        assert_eq!(
            language_id_for_path(Path::new("script.sh")),
            Some("shellscript")
        );
        assert_eq!(
            language_id_for_path(Path::new("run.bash")),
            Some("shellscript")
        );
        assert_eq!(
            language_id_for_path(Path::new("init.zsh")),
            Some("shellscript")
        );
    }

    #[test]
    fn config_formats() {
        assert_eq!(language_id_for_path(Path::new("config.toml")), Some("toml"));
        assert_eq!(language_id_for_path(Path::new("config.yaml")), Some("yaml"));
        assert_eq!(language_id_for_path(Path::new("config.yml")), Some("yaml"));
        assert_eq!(language_id_for_path(Path::new("data.json")), Some("json"));
    }

    #[test]
    fn web_extensions() {
        assert_eq!(language_id_for_path(Path::new("index.html")), Some("html"));
        assert_eq!(language_id_for_path(Path::new("style.css")), Some("css"));
        assert_eq!(
            language_id_for_path(Path::new("README.md")),
            Some("markdown")
        );
    }

    #[test]
    fn other_languages() {
        assert_eq!(language_id_for_path(Path::new("init.lua")), Some("lua"));
        assert_eq!(language_id_for_path(Path::new("Main.java")), Some("java"));
        assert_eq!(language_id_for_path(Path::new("app.rb")), Some("ruby"));
    }

    #[test]
    fn terraform_extensions() {
        assert_eq!(
            language_id_for_path(Path::new("main.tf")),
            Some("terraform")
        );
        assert_eq!(
            language_id_for_path(Path::new("variables.tfvars")),
            Some("terraform")
        );
        assert_eq!(
            language_id_for_path(Path::new("terragrunt.hcl")),
            Some("terraform")
        );
    }

    #[test]
    fn unknown_extension_returns_none() {
        assert_eq!(language_id_for_path(Path::new("file.xyz")), None);
        assert_eq!(language_id_for_path(Path::new("archive.tar")), None);
    }

    #[test]
    fn no_extension_returns_none() {
        assert_eq!(language_id_for_path(Path::new("Makefile")), None);
        assert_eq!(language_id_for_path(Path::new("Dockerfile")), None);
    }

    #[test]
    fn path_with_directories() {
        assert_eq!(
            language_id_for_path(&PathBuf::from("/home/user/project/src/main.rs")),
            Some("rust")
        );
    }

    #[test]
    fn dotfile_without_extension_returns_none() {
        assert_eq!(language_id_for_path(Path::new(".gitignore")), None);
        assert_eq!(language_id_for_path(Path::new(".env")), None);
    }

    #[test]
    fn multiple_dots_in_filename_uses_last_extension() {
        assert_eq!(
            language_id_for_path(Path::new("my.component.tsx")),
            Some("typescript")
        );
        assert_eq!(language_id_for_path(Path::new("archive.tar.gz")), None);
        assert_eq!(
            language_id_for_path(Path::new("config.backup.json")),
            Some("json")
        );
    }

    #[test]
    fn extension_is_case_sensitive() {
        // Rust's Path::extension preserves case, and our match is lowercase-only.
        assert_eq!(language_id_for_path(Path::new("Main.RS")), None);
        assert_eq!(language_id_for_path(Path::new("App.PY")), None);
        assert_eq!(language_id_for_path(Path::new("index.HTML")), None);
    }

    #[test]
    fn empty_path_returns_none() {
        assert_eq!(language_id_for_path(Path::new("")), None);
    }

    #[test]
    fn directory_like_path_no_extension_returns_none() {
        assert_eq!(language_id_for_path(Path::new("/usr/bin/")), None);
    }

    #[test]
    fn hidden_file_with_known_extension() {
        assert_eq!(language_id_for_path(Path::new(".hidden.rs")), Some("rust"));
        assert_eq!(
            language_id_for_path(Path::new(".config.json")),
            Some("json")
        );
    }
}
