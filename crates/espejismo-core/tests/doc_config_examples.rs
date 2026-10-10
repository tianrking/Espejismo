use std::{fs, path::Path};

fn markdown_files(path: &Path, files: &mut Vec<std::path::PathBuf>) {
    if path.is_dir() {
        for entry in fs::read_dir(path).expect("read documentation directory") {
            markdown_files(&entry.expect("read directory entry").path(), files);
        }
    } else if path.extension().is_some_and(|extension| extension == "md") {
        files.push(path.to_path_buf());
    }
}

#[test]
fn documented_toml_examples_parse_as_config() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = vec![root.join("README.md"), root.join("README_ES.md")];
    markdown_files(&root.join("docs"), &mut files);

    let mut checked = 0;
    for path in files {
        let source = fs::read_to_string(&path).expect("read Markdown file");
        let mut in_toml = false;
        let mut example = String::new();
        let mut start_line = 0;
        for (index, line) in source.lines().enumerate() {
            if !in_toml && line.trim() == "```toml" {
                in_toml = true;
                start_line = index + 1;
                example.clear();
            } else if in_toml && line.trim() == "```" {
                let relative = path.strip_prefix(&root).unwrap_or(&path);
                espejismo_core::parse_config(&example).unwrap_or_else(|error| {
                    panic!(
                        "{}:{} TOML example does not parse: {error}\n{example}",
                        relative.display(),
                        start_line
                    )
                });
                checked += 1;
                in_toml = false;
            } else if in_toml {
                example.push_str(line);
                example.push('\n');
            }
        }
        assert!(!in_toml, "unterminated TOML fence in {}", path.display());
    }
    assert!(checked > 0, "no documented TOML examples found");
}
