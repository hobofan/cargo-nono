use proc_macro2;
use proc_macro2::TokenTree;
use quote::quote;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use ext::*;

#[derive(Debug, PartialEq, Eq)]
pub enum CrateSupport {
    OnlyWithoutFeature(String),
    /// proc macros are not actually linked, so they don't hinder no_std support
    ProcMacro,
    SourceOffenses(Vec<SourceOffense>),
    NoOffenseDetected,
}

#[derive(Debug)]
struct ConditionalAttribute {
    condition: proc_macro2::TokenStream,
    attribute: syn::Ident,
}

impl ConditionalAttribute {
    fn from_attribute(attr: &syn::Attribute) -> Option<Self> {
        let cfg_attr_path: syn::Path = syn::parse_quote!(cfg_attr);
        if attr.path == cfg_attr_path {
            if let Some(ref first_group_ts) = attr.clone().tts.into_iter().next() {
                // Group of the surrounding parenthesis
                if let TokenTree::Group(group) = first_group_ts {
                    let mut inner_group_stream = group.stream().into_iter();
                    let condition_part_1 = inner_group_stream.next();
                    let condition_part_2 = inner_group_stream.next();
                    inner_group_stream.next();
                    let gated_attr = inner_group_stream.next();

                    if let Some(TokenTree::Ident(ref gated_attr_ident)) = gated_attr {
                        let mut condition = proc_macro2::TokenStream::new();
                        condition.extend(condition_part_1);
                        condition.extend(condition_part_2);

                        return Some(ConditionalAttribute {
                            condition,
                            attribute: gated_attr_ident.clone(),
                        });
                    }
                }
            }
        }
        return None;
    }

    fn required_feature(&self) -> Option<proc_macro2::Literal> {
        let not_ident: syn::Ident = syn::parse_quote!(not);
        let feature_ident: syn::Ident = syn::parse_quote!(feature);
        let equal_punct: proc_macro2::Punct = syn::parse_quote!(=);

        let mut ts = self.condition.clone().into_iter();
        if let Some(TokenTree::Ident(not_ident_parsed)) = ts.next() {
            if not_ident == not_ident_parsed {
                if let Some(TokenTree::Group(group_parsed)) = ts.next() {
                    let mut group_stream = group_parsed.stream().into_iter();
                    let feat_ident = group_stream.next();
                    let eq_punct = group_stream.next();
                    let required_literal = group_stream.next();

                    if let (
                        Some(TokenTree::Ident(feat_ident_parsed)),
                        Some(TokenTree::Punct(equal_punct_parsed)),
                        Some(TokenTree::Literal(req_literal)),
                    ) = (feat_ident, eq_punct, required_literal)
                    {
                        if feature_ident == feat_ident_parsed
                            && equal_punct.as_char() == equal_punct_parsed.as_char()
                        {
                            return Some(req_literal);
                        }
                    }
                }
            }
        }
        return None;
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SourceOffense {
    /// Source code is missing a `#![no_std]` attribute.
    /// Only valid for entry point file (main.rs / lib.rs).
    MissingNoStdAttribute,
    /// Source code contains an explicit `use std::` statement.
    UseStdStatement,
}

pub fn get_crate_support_from_source(src_path: &PathBuf) -> CrateSupport {
    let mut file = File::open(&src_path).expect("Unable to open file");

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

    let use_statements: Vec<_> = syntax.items.iter().filter_map(|item| match item {
        syn::Item::Use(item) => Some(item),
        _ => None,
    }).collect();

    let mut has_use_std = false;
    let std_ident: syn::Ident = syn::parse_quote!(std);
    for use_statement in &use_statements {
        match use_statement.tree {
            syn::UseTree::Path(ref first_path) => {
                let first_ident = &first_path.ident;
                if first_ident == &std_ident {
                    has_use_std = true;
                }
            },
            _ => unimplemented!(),
        }
    }
    if has_use_std {
        offenses.push(SourceOffense::UseStdStatement);
    }

    let always_no_std: syn::Attribute = syn::parse_quote!(#![no_std]);
    let contains_always_no_std = syntax.attrs.contains(&always_no_std);
    if !contains_always_no_std {
        offenses.push(SourceOffense::MissingNoStdAttribute);
    }

    match offenses.is_empty() {
        true => CrateSupport::NoOffenseDetected,
        false => CrateSupport::SourceOffenses(offenses),
    }
}

pub struct CheckResult {
    pub package_name: String,
    pub support: CrateSupport,
    pub active_features: Vec<Feature>,
}

impl CheckResult {
    pub fn no_std_itself(&self) -> bool {
        match self.support {
            CrateSupport::ProcMacro => true,
            CrateSupport::OnlyWithoutFeature(ref feature) => !self.is_feature_active(feature),
            CrateSupport::NoOffenseDetected => true,
            CrateSupport::SourceOffenses(_) => false,
        }
    }

    pub fn is_feature_active(&self, feature: &str) -> bool {
        self.find_active_feature_by_name(feature).is_some()
    }

    pub fn find_active_feature_by_name(&self, feature: &str) -> Option<&Feature> {
        self.active_features.iter().find(|n| &n.name == feature)
    }
}
