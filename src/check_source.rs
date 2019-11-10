use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

#[cfg(feature = "proc_macro_spans")]
use std::io::BufRead;
#[cfg(feature = "proc_macro_spans")]
use syn::spanned::Spanned;

use crate::check::*;

#[derive(Debug, PartialEq, Eq)]
pub enum SourceOffense {
    /// Source code is missing a `#![no_std]` attribute.
    /// Only valid for entry point file (main.rs / lib.rs).
    MissingNoStdAttribute,
    /// Source code contains an explicit `use std::` statement.
    UseStdStatement(UseStdStmt),
}

#[derive(Debug)]
pub struct UseStdStmt {
    src_path: PathBuf,
    item_tree: syn::UseTree,
}

impl UseStdStmt {
    #[cfg(feature = "proc_macro_spans")]
    // TODO: can be made available without proc_macro_spans by parsing from UseTree
    fn statement_str(&self) -> String {
        let file = File::open(&self.src_path).unwrap();
        let file = std::io::BufReader::new(file);
        let line = file
            .lines()
            .skip(self.item_tree.span().start().line - 1)
            .next()
            .unwrap()
            .unwrap();

        let raw_part: String = line
            .chars()
            .skip(self.item_tree.span().start().column)
            .take(self.item_tree.span().end().column - self.item_tree.span().start().column)
            .collect();

        raw_part
    }

    #[cfg(feature = "proc_macro_spans")]
    /// `std::path::PathBuf` -> `["std", "path", "PathBuf"]`
    fn path_parts(&self) -> Vec<String> {
        let raw_part = self.statement_str();
        raw_part.split("::").map(|n| n.to_owned()).collect()
    }
}

impl PartialEq for UseStdStmt {
    fn eq(&self, other: &UseStdStmt) -> bool {
        self.src_path == other.src_path
    }
}
impl Eq for UseStdStmt {}

#[cfg(feature = "proc_macro_spans")]
impl fmt::Display for UseStdStmt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let file = File::open(&self.src_path).unwrap();
        let file = std::io::BufReader::new(file);
        let line = file
            .lines()
            .skip(self.item_tree.span().start().line - 1)
            .next()
            .unwrap()
            .unwrap();

        let statement_str = self.statement_str();

        let replacement_suggestion = find_use_std_statement_replacement(&self.path_parts());
        let replacement_suggestion = replacement_suggestion.map(|n| n.join("::"));

        writeln!(
            f,
            "   --> {src}:{line}:{column}",
            src = self
                .src_path
                .strip_prefix(std::env::current_dir().unwrap())
                .unwrap()
                .display(),
            line = self.item_tree.span().start().line,
            column = self.item_tree.span().start().column
        )?;
        writeln!(f, "    |")?;

        writeln!(
            f,
            "{line_num:<4}|{line}",
            line_num = self.item_tree.span().start().line,
            line = line
        )?;

        let mut underline: Vec<char> = vec![];
        for _ in 0..self.item_tree.span().start().column {
            underline.push(' ');
        }
        for _ in self.item_tree.span().start().column..self.item_tree.span().end().column {
            underline.push('^');
        }
        let underline: String = underline.into_iter().collect();

        writeln!(f, "    |{line}", line = underline)?;
        if let Some(replacement_suggestion) = replacement_suggestion {
            writeln!(
                f,
                "help: Try replacing `{original}` with `{replacement}`.",
                original = statement_str,
                replacement = replacement_suggestion
            )?;
        }
        Ok(())
    }
}

#[cfg(not(feature = "proc_macro_spans"))]
impl fmt::Display for UseStdStmt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "   --> {src}",
            src = self
                .src_path
                .strip_prefix(std::env::current_dir().unwrap())
                .unwrap_or(&self.src_path)
                .display(),
        )
    }
}

pub fn get_crate_support_from_source(main_src_path: &PathBuf) -> CrateSupport {
    let main_file_support = check_source(main_src_path, true);

    let mut offenses = vec![];
    match main_file_support {
        CrateSupport::OnlyWithoutFeature(_) => return main_file_support,
        CrateSupport::ProcMacro => return main_file_support,
        CrateSupport::SourceOffenses(mut off) => offenses.append(&mut off),
        CrateSupport::NoOffenseDetected => {}
    };

    let other_source_files_pattern = format!(
        "{}/**/*.rs",
        main_src_path.parent().unwrap().to_str().unwrap(),
    );
    let other_source_files = glob::glob(&other_source_files_pattern).unwrap();

    for other_source_file in other_source_files {
        let file_support = check_source(&other_source_file.unwrap(), false);
        match file_support {
            CrateSupport::SourceOffenses(mut off) => offenses.append(&mut off),
            _ => {}
        }
    }

    match offenses.is_empty() {
        true => CrateSupport::NoOffenseDetected,
        false => CrateSupport::SourceOffenses(offenses),
    }
}

fn check_source(source_path: &PathBuf, is_main_file: bool) -> CrateSupport {
    let mut file = File::open(&source_path).expect("Unable to open file");

    let mut src = String::new();
    file.read_to_string(&mut src).expect("Unable to read file");

    let syntax = syn::parse_file(&src).expect("Unable to parse file");

    for attr in &syntax.attrs {
        if let Some(conditional_attr) = ConditionalAttribute::from_attribute(&attr) {
            let no_std_ident: syn::Ident = syn::parse_quote!(no_std);
            if conditional_attr.attribute == no_std_ident {
                if let Some(required_feature) = conditional_attr.required_feature() {
                    let mut feature_name = required_feature.to_string();
                    feature_name = feature_name[1..feature_name.len() - 1].to_owned();
                    return CrateSupport::OnlyWithoutFeature(feature_name);
                }
            }
        }
    }

    let mut offenses = vec![];

    let use_statements: Vec<_> = syntax
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Use(item) => Some(item),
            _ => None,
        })
        .collect();

    let std_ident: syn::Ident = syn::parse_quote!(std);
    for use_statement in &use_statements {
        match use_statement.tree {
            syn::UseTree::Path(ref first_path) => {
                let first_ident = &first_path.ident;
                if first_ident == &std_ident {
                    let stmt = UseStdStmt {
                        src_path: source_path.clone(),
                        item_tree: use_statement.tree.clone(),
                    };
                    offenses.push(SourceOffense::UseStdStatement(stmt));
                }
            }
            _ => {
                // FIXME: #19 - ignore non-trivial use statements for now
            }
        }
    }

    if is_main_file {
        let always_no_std: syn::Attribute = syn::parse_quote!(#![no_std]);
        let contains_always_no_std = syntax.attrs.contains(&always_no_std);
        if !contains_always_no_std {
            let not_test_no_std: syn::Attribute = syn::parse_quote!(#![cfg_attr(not(test), no_std)]);
            let contains_not_test_no_std = syntax.attrs.contains(&not_test_no_std);
            if !contains_not_test_no_std {
                offenses.push(SourceOffense::MissingNoStdAttribute);
            }
        }
    }

    match offenses.is_empty() {
        true => CrateSupport::NoOffenseDetected,
        false => CrateSupport::SourceOffenses(offenses),
    }
}

/// Really hacky way of trying to find a replacment for a `use std::` statment.
///
/// Right now checks if `std` can be replaced by `core` in the following way:
/// - Find out directory of `rustdoc` via `rustup which rustdoc`
/// - Infer rust doc directory for that
/// - Try to find a file in `core` docs that would serve as replacment for `std` item
#[allow(dead_code)]
fn find_use_std_statement_replacement(path_parts: &[String]) -> Option<Vec<String>> {
    let rustup_output = std::process::Command::new("rustup")
        .args(&vec!["which", "rustdoc"])
        .output()
        .expect("failed to execute rustup");
    let rustdoc_dir = String::from_utf8(rustup_output.stdout).unwrap();
    let mut doc_dir = PathBuf::from(&rustdoc_dir);
    doc_dir.pop();
    doc_dir = doc_dir.join("../share/doc/rust/html");

    let mut core_dir = doc_dir.join("core");
    for path_part in path_parts.iter().skip(1).take(path_parts.len() - 2) {
        core_dir = core_dir.join(path_part);
    }

    let glob_pattern = format!(
        "{}/*.{}.html",
        core_dir.to_str().unwrap(),
        path_parts.last().unwrap()
    );
    let mut glob_files = glob::glob(&glob_pattern).unwrap();
    match glob_files.next().is_some() {
        true => {
            let replacement_path = vec!["core".to_owned()]
                .into_iter()
                .chain(path_parts.into_iter().skip(1).map(|n| n.clone()))
                .collect();
            return Some(replacement_path);
        }
        false => {}
    };

    // check for module index files, so that module use statments like `use std::ops`
    // are checked correctly
    let glob_pattern_index = format!(
        "{}/{}/index.html",
        core_dir.to_str().unwrap(),
        path_parts.last().unwrap()
    );
    let mut glob_files = glob::glob(&glob_pattern_index).unwrap();
    match glob_files.next().is_some() {
        true => {
            let replacement_path = vec!["core".to_owned()]
                .into_iter()
                .chain(path_parts.into_iter().skip(1).map(|n| n.clone()))
                .collect();
            return Some(replacement_path);
        }
        false => None,
    }
}
