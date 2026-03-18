//! File type icon mappings using Nerd Font characters.
//!
//! Provides icon and color pairs for files based on filename or extension.
//! Icons are Nerd Font glyphs — requires a Nerd Font-patched terminal font.

use ratatui::style::Color;

/// An icon glyph with its associated display color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIcon {
    /// The Nerd Font glyph string (includes trailing space for padding).
    pub icon: &'static str,
    /// The color to render the icon in.
    pub color: Color,
}

/// Default icon for files with no matching extension.
pub const DEFAULT_FILE_ICON: FileIcon = FileIcon {
    icon: "󰈔 ",
    color: Color::Rgb(124, 139, 157),
};

/// Icon for collapsed directories.
pub const DIR_CLOSED_ICON: FileIcon = FileIcon {
    icon: "󰉋 ",
    color: Color::Rgb(86, 182, 194),
};

/// Icon for expanded directories.
pub const DIR_OPEN_ICON: FileIcon = FileIcon {
    icon: "󰉖 ",
    color: Color::Rgb(86, 182, 194),
};

/// Icon for symlinks.
pub const SYMLINK_ICON: FileIcon = FileIcon {
    icon: "󰌷 ",
    color: Color::Rgb(198, 120, 221),
};

// IMPACT ANALYSIS — icon_for_file
// Parents: icon_for_node() in axe-ui calls this for File nodes during tree rendering.
// Children: extension_from(), icon_for_extension() — internal helpers.
// Siblings: DIR_OPEN_ICON/DIR_CLOSED_ICON/SYMLINK_ICON — used for non-file nodes.

/// Returns the appropriate icon for a file based on its name and extension.
///
/// Checks special filenames first (case-insensitive), then falls back to
/// extension-based lookup. Returns `DEFAULT_FILE_ICON` if no match found.
pub fn icon_for_file(filename: &str) -> FileIcon {
    let lower = filename.to_ascii_lowercase();

    // Special filenames (checked first).
    match lower.as_str() {
        "cargo.toml" | "cargo.lock" => FileIcon {
            icon: " ",
            color: Color::Rgb(222, 165, 77),
        },
        "makefile" => FileIcon {
            icon: " ",
            color: Color::Rgb(111, 150, 67),
        },
        "dockerfile" | "containerfile" => FileIcon {
            icon: "󰡨 ",
            color: Color::Rgb(56, 145, 211),
        },
        ".gitignore" | ".gitmodules" | ".gitattributes" => FileIcon {
            icon: " ",
            color: Color::Rgb(222, 77, 52),
        },
        "package.json" | "package-lock.json" => FileIcon {
            icon: " ",
            color: Color::Rgb(68, 163, 66),
        },
        "tsconfig.json" => FileIcon {
            icon: " ",
            color: Color::Rgb(56, 145, 211),
        },
        "go.mod" | "go.sum" => FileIcon {
            icon: " ",
            color: Color::Rgb(0, 173, 216),
        },
        "readme.md" | "readme" => FileIcon {
            icon: "󰂺 ",
            color: Color::Rgb(66, 165, 245),
        },
        "license" | "license.md" | "licence" | "licence.md" => FileIcon {
            icon: "󰿃 ",
            color: Color::Rgb(205, 177, 59),
        },
        ".env" | ".env.local" | ".env.production" | ".env.development" => FileIcon {
            icon: " ",
            color: Color::Rgb(250, 200, 50),
        },
        _ => icon_for_extension(extension_from(filename)),
    }
}

/// Extracts the lowercase extension from a filename.
fn extension_from(filename: &str) -> &str {
    filename.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("")
}

/// Returns the icon for a given file extension (lowercase).
fn icon_for_extension(ext: &str) -> FileIcon {
    let ext_lower = ext.to_ascii_lowercase();
    match ext_lower.as_str() {
        // Rust
        "rs" => FileIcon {
            icon: " ",
            color: Color::Rgb(222, 165, 77),
        },
        // Go
        "go" => FileIcon {
            icon: " ",
            color: Color::Rgb(0, 173, 216),
        },
        // Python
        "py" | "pyi" | "pyw" => FileIcon {
            icon: " ",
            color: Color::Rgb(55, 118, 171),
        },
        // JavaScript
        "js" | "mjs" | "cjs" => FileIcon {
            icon: " ",
            color: Color::Rgb(241, 224, 90),
        },
        // TypeScript
        "ts" | "mts" | "cts" => FileIcon {
            icon: " ",
            color: Color::Rgb(56, 145, 211),
        },
        // TSX / JSX
        "tsx" => FileIcon {
            icon: " ",
            color: Color::Rgb(56, 145, 211),
        },
        "jsx" => FileIcon {
            icon: " ",
            color: Color::Rgb(241, 224, 90),
        },
        // Java
        "java" => FileIcon {
            icon: " ",
            color: Color::Rgb(204, 62, 68),
        },
        // C
        "c" => FileIcon {
            icon: " ",
            color: Color::Rgb(85, 135, 195),
        },
        // C++
        "cpp" | "cc" | "cxx" => FileIcon {
            icon: " ",
            color: Color::Rgb(85, 135, 195),
        },
        // C/C++ headers
        "h" | "hpp" | "hxx" => FileIcon {
            icon: " ",
            color: Color::Rgb(146, 116, 203),
        },
        // C#
        "cs" => FileIcon {
            icon: "󰌛 ",
            color: Color::Rgb(104, 33, 122),
        },
        // Ruby
        "rb" => FileIcon {
            icon: " ",
            color: Color::Rgb(204, 52, 45),
        },
        // PHP
        "php" => FileIcon {
            icon: " ",
            color: Color::Rgb(119, 123, 179),
        },
        // Swift
        "swift" => FileIcon {
            icon: " ",
            color: Color::Rgb(240, 81, 56),
        },
        // Kotlin
        "kt" | "kts" => FileIcon {
            icon: " ",
            color: Color::Rgb(126, 87, 194),
        },
        // Lua
        "lua" => FileIcon {
            icon: " ",
            color: Color::Rgb(0, 0, 128),
        },
        // Zig
        "zig" => FileIcon {
            icon: " ",
            color: Color::Rgb(236, 145, 30),
        },
        // Terraform / HCL
        "tf" | "tfvars" | "hcl" => FileIcon {
            icon: "󱁢 ",
            color: Color::Rgb(99, 72, 188),
        },
        // Shell
        "sh" | "bash" | "zsh" | "fish" => FileIcon {
            icon: " ",
            color: Color::Rgb(137, 224, 81),
        },
        // HTML
        "html" | "htm" => FileIcon {
            icon: " ",
            color: Color::Rgb(227, 76, 38),
        },
        // CSS
        "css" => FileIcon {
            icon: " ",
            color: Color::Rgb(66, 165, 245),
        },
        // SCSS / Sass
        "scss" | "sass" => FileIcon {
            icon: " ",
            color: Color::Rgb(205, 103, 153),
        },
        // JSON
        "json" | "jsonc" => FileIcon {
            icon: " ",
            color: Color::Rgb(241, 224, 90),
        },
        // XML
        "xml" => FileIcon {
            icon: "󰗀 ",
            color: Color::Rgb(227, 76, 38),
        },
        // YAML
        "yaml" | "yml" => FileIcon {
            icon: " ",
            color: Color::Rgb(204, 62, 68),
        },
        // TOML
        "toml" => FileIcon {
            icon: " ",
            color: Color::Rgb(111, 150, 67),
        },
        // SVG
        "svg" => FileIcon {
            icon: "󰜡 ",
            color: Color::Rgb(255, 179, 0),
        },
        // Markdown
        "md" | "mdx" => FileIcon {
            icon: "󰍔 ",
            color: Color::Rgb(66, 165, 245),
        },
        // Text
        "txt" => FileIcon {
            icon: "󰈙 ",
            color: Color::Rgb(137, 171, 79),
        },
        // PDF
        "pdf" => FileIcon {
            icon: "󰈦 ",
            color: Color::Rgb(204, 52, 45),
        },
        // SQL
        "sql" => FileIcon {
            icon: " ",
            color: Color::Rgb(218, 208, 133),
        },
        // CSV
        "csv" => FileIcon {
            icon: "󰈙 ",
            color: Color::Rgb(137, 171, 79),
        },
        // Lock files
        "lock" => FileIcon {
            icon: " ",
            color: Color::Rgb(124, 139, 157),
        },
        // Files ending in .dockerfile
        "dockerfile" => FileIcon {
            icon: "󰡨 ",
            color: Color::Rgb(56, 145, 211),
        },
        // Images
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "webp" => FileIcon {
            icon: " ",
            color: Color::Rgb(160, 116, 196),
        },
        // Default
        _ => DEFAULT_FILE_ICON,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_for_rust_file() {
        let icon = icon_for_file("main.rs");
        assert_eq!(icon.icon, " ");
        assert_ne!(icon, DEFAULT_FILE_ICON);
    }

    #[test]
    fn icon_for_directory_icons_exist() {
        assert!(!DIR_CLOSED_ICON.icon.is_empty());
        assert!(!DIR_OPEN_ICON.icon.is_empty());
        assert_ne!(DIR_CLOSED_ICON.icon, DIR_OPEN_ICON.icon);
    }

    #[test]
    fn icon_for_unknown_extension() {
        let icon = icon_for_file("data.xyzabc");
        assert_eq!(icon, DEFAULT_FILE_ICON);
    }

    #[test]
    fn icon_for_special_file() {
        let icon = icon_for_file("Cargo.toml");
        // Special file match, not generic .toml extension.
        assert_eq!(icon.icon, " ");
    }

    #[test]
    fn icon_for_no_extension() {
        let icon = icon_for_file("LICENSE");
        // LICENSE is a special filename match.
        assert_ne!(icon.icon, DEFAULT_FILE_ICON.icon);

        // A truly unknown file with no extension.
        let icon2 = icon_for_file("randomfile");
        assert_eq!(icon2, DEFAULT_FILE_ICON);
    }

    #[test]
    fn icon_for_case_insensitive_special_files() {
        let icon1 = icon_for_file("Makefile");
        let icon2 = icon_for_file("makefile");
        assert_eq!(icon1, icon2);
        assert_eq!(icon1.icon, " ");
    }

    #[test]
    fn icon_for_common_languages() {
        // Verify at least 30 extensions are mapped by checking a selection.
        let cases = vec![
            ("test.rs", " "),
            ("test.go", " "),
            ("test.py", " "),
            ("test.js", " "),
            ("test.ts", " "),
            ("test.java", " "),
            ("test.c", " "),
            ("test.cpp", " "),
            ("test.rb", " "),
            ("test.php", " "),
            ("test.swift", " "),
            ("test.html", " "),
            ("test.css", " "),
            ("test.json", " "),
            ("test.yaml", " "),
            ("test.md", "󰍔 "),
            ("test.sql", " "),
            ("test.sh", " "),
        ];
        for (filename, expected_icon) in cases {
            let icon = icon_for_file(filename);
            assert_eq!(
                icon.icon, expected_icon,
                "Wrong icon for {filename}: got '{}', expected '{expected_icon}'",
                icon.icon
            );
        }
    }

    #[test]
    fn symlink_icon_exists() {
        assert!(!SYMLINK_ICON.icon.is_empty());
        assert_ne!(SYMLINK_ICON, DEFAULT_FILE_ICON);
    }

    #[test]
    fn icon_for_gitignore() {
        let icon = icon_for_file(".gitignore");
        assert_eq!(icon.icon, " ");
    }

    #[test]
    fn icon_for_dockerfile() {
        let icon = icon_for_file("Dockerfile");
        assert_eq!(icon.icon, "󰡨 ");
    }
}
