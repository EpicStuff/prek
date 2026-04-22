/* MIT License

Copyright (c) 2023 Astral Software Inc.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fmt::Write as _};

use fs_err as fs;

fn main() {
    // The workspace root directory is not available without walking up the tree
    // https://github.com/rust-lang/cargo/issues/3946
    #[allow(clippy::disallowed_methods)]
    let workspace_root = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .expect("CARGO_MANIFEST_DIR should be nested in workspace")
        .parent()
        .expect("CARGO_MANIFEST_DIR should be doubly nested in workspace")
        .to_path_buf();

    commit_info(&workspace_root);
    build_embedded_adaptors(&workspace_root);
}

fn commit_info(workspace_root: &Path) {
    // If not in a git repository, do not attempt to retrieve commit information
    let git_dir = workspace_root.join(".git");
    if !git_dir.exists() {
        return;
    }

    if let Some(git_head_path) = git_head(&git_dir) {
        println!("cargo:rerun-if-changed={}", git_head_path.display());

        let git_head_contents = fs::read_to_string(git_head_path);
        if let Ok(git_head_contents) = git_head_contents {
            // The contents are either a commit or a reference in the following formats
            // - "<commit>" when the head is detached
            // - "ref <ref>" when working on a branch
            // If a commit, checking if the HEAD file has changed is sufficient
            // If a ref, we need to add the head file for that ref to rebuild on commit
            let mut git_ref_parts = git_head_contents.split_whitespace();
            git_ref_parts.next();
            if let Some(git_ref) = git_ref_parts.next() {
                let git_ref_path = git_dir.join(git_ref);
                println!("cargo:rerun-if-changed={}", git_ref_path.display());
            }
        }
    }

    let output = match Command::new("git")
        .arg("log")
        .arg("-1")
        .arg("--date=short")
        .arg("--abbrev=9")
        // describe:tags => Instead of only considering annotated tags, consider lightweight tags as well.
        .arg("--format='%H %h %cd %(describe:tags)'")
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return,
    };
    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut parts = stdout.split_whitespace();
    let mut next = || parts.next().unwrap();
    println!("cargo:rustc-env=PREK_COMMIT_HASH={}", next());
    println!("cargo:rustc-env=PREK_COMMIT_SHORT_HASH={}", next());
    println!("cargo:rustc-env=PREK_COMMIT_DATE={}", next());

    // Describe can fail for some commits
    // https://git-scm.com/docs/pretty-formats#Documentation/pretty-formats.txt-emdescribeoptionsem
    if let Some(describe) = parts.next() {
        // e.g. 'v0.2.0-alpha.5-1-g4e9faf2'
        let mut describe_parts = describe.rsplitn(3, '-');
        describe_parts.next();
        println!(
            "cargo:rustc-env=PREK_LAST_TAG_DISTANCE={}",
            describe_parts.next().unwrap_or("0")
        );
        if let Some(last_tag) = describe_parts.next() {
            println!("cargo:rustc-env=PREK_LAST_TAG={last_tag}");
        }
    }
}

fn git_head(git_dir: &Path) -> Option<PathBuf> {
    // The typical case is a standard git repository.
    let git_head_path = git_dir.join("HEAD");
    if git_head_path.exists() {
        return Some(git_head_path);
    }
    if !git_dir.is_file() {
        return None;
    }
    // If `.git/HEAD` doesn't exist and `.git` is actually a file,
    // then let's try to attempt to read it as a worktree. If it's
    // a worktree, then its contents will look like this, e.g.:
    //
    //     gitdir: /home/andrew/astral/uv/main/.git/worktrees/pr2
    //
    // And the HEAD file we want to watch will be at:
    //
    //     /home/andrew/astral/uv/main/.git/worktrees/pr2/HEAD
    let contents = fs::read_to_string(git_dir).ok()?;
    let (label, worktree_path) = contents.split_once(':')?;
    if label != "gitdir" {
        return None;
    }
    let worktree_path = worktree_path.trim();
    Some(PathBuf::from(worktree_path))
}

fn build_embedded_adaptors(workspace_root: &Path) {
    let adaptors_dir = workspace_root.join("adaptors");
    println!("cargo:rerun-if-changed={}", adaptors_dir.display());

    if !adaptors_dir.exists() {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR should be set"));
    let compiled_dir = out_dir.join("embedded_adaptors");
    fs::create_dir_all(&compiled_dir).expect("Failed to create embedded adaptor output directory");
    let nim_available = Command::new("nim").arg("--version").output().is_ok();

    let mut entries = Vec::<(String, String, PathBuf)>::new();
    let mut yaml_entries = Vec::<(String, String)>::new();
    let mut queued = vec![adaptors_dir.clone()];

    while let Some(dir) = queued.pop() {
        let read = fs::read_dir(&dir).unwrap_or_else(|e| {
            panic!("Failed to read adaptors directory `{}`: {e}", dir.display())
        });

        for entry in read.flatten() {
            let path = entry.path();
            if path.is_dir() {
                queued.push(path.clone());
                println!("cargo:rerun-if-changed={}", path.display());
                continue;
            }

            let ext = path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();

            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("Adaptor file name should be valid UTF-8")
                .to_string();
            println!("cargo:rerun-if-changed={}", path.display());

            if ext == "nim" {
                let mut output_name = stem.clone();
                if cfg!(windows) {
                    output_name.push_str(".exe");
                }
                let output_path = compiled_dir.join(&output_name);
                if !nim_available {
                    panic!(
                        "Failed to compile Nim adaptor `{}` because `nim` is not available on PATH during build",
                        path.display()
                    );
                }
                compile_nim_adaptor(&path, &output_path);
                entries.push((stem, output_name, output_path));
            } else if ext == "yaml" {
                let content = fs::read_to_string(&path).unwrap_or_else(|e| {
                    panic!("Failed to read adaptor yaml `{}`: {e}", path.display())
                });
                yaml_entries.push((stem, content));
            }
        }
    }

    generate_embedded_adaptors_rs(&out_dir.join("embedded_adaptors.rs"), &entries, &yaml_entries);
}

fn compile_nim_adaptor(source: &Path, output: &Path) {
    let status = Command::new("nim")
        .arg("c")
        .arg("-d:release")
        .arg("--opt:size")
        .arg(format!("--out:{}", output.display()))
        .arg(source)
        .status();
    match status {
        Ok(status) if status.success() => {}
        Ok(status) => panic!(
            "Failed to compile Nim adaptor `{}`: nim exited with {status}",
            source.display()
        ),
        Err(err) => panic!(
            "Failed to invoke Nim compiler while compiling adaptor `{}`: {err}",
            source.display()
        ),
    }
}

fn generate_embedded_adaptors_rs(
    path: &Path,
    entries: &[(String, String, PathBuf)],
    yaml_entries: &[(String, String)],
) {
    let mut content = String::new();
    content.push_str("pub(crate) const EMBEDDED_ADAPTOR_NAMES: &[&str] = &[\n");
    for (name, _, _) in entries {
        let _ = writeln!(content, "    {name:?},");
    }
    content.push_str("];\n\n");
    content.push_str("pub(crate) fn embedded_adaptor_yaml(name: &str) -> Option<&'static str> {\n");
    content.push_str("    match name {\n");
    for (name, yaml) in yaml_entries {
        let _ = writeln!(content, "        {name:?} => Some({yaml:?}),");
    }
    content.push_str("        _ => None,\n");
    content.push_str("    }\n");
    content.push_str("}\n\n");
    content.push_str("pub(crate) fn embedded_adaptor(name: &str) -> Option<(&'static str, &'static [u8])> {\n");
    content.push_str("    match name {\n");
    for (name, output_name, output_path) in entries {
        let _ = writeln!(
            content,
            "        {name:?} => Some(({output_name:?}, include_bytes!({:?}))),",
            output_path.display().to_string()
        );
    }
    content.push_str("        _ => None,\n");
    content.push_str("    }\n");
    content.push_str("}\n");
    fs::write(path, content).expect("Failed to write embedded adaptor metadata");
}
