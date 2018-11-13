use quote::quote;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use ext::*;

#[derive(Debug, PartialEq, Eq)]
pub enum CrateSupport {
    AlwaysNoStd,
    OnlyWithoutFeature(String),
    /// proc macros are not actually linked, so they don't hinder no_std support
    ProcMacro,
    NotDetected,
}

pub fn get_crate_support_from_source(src_path: &PathBuf) -> CrateSupport {
    let mut file = File::open(&src_path).expect("Unable to open file");

    let mut src = String::new();
    file.read_to_string(&mut src).expect("Unable to read file");

    let syntax = syn::parse_file(&src).expect("Unable to parse file");

    let known_feature_names = vec!["std", "use_std"];
    for known_feature_name in known_feature_names {
        let only_without_std_feature: syn::Attribute =
            syn::parse_quote!(#![cfg_attr(not(feature = #known_feature_name), no_std)]);
        let contains_only_without = syntax.attrs.contains(&only_without_std_feature);
        if contains_only_without {
            return CrateSupport::OnlyWithoutFeature(known_feature_name.to_owned());
        }
    }

    let always_no_std: syn::Attribute = syn::parse_quote!(#![no_std]);
    let contains_always_no_std = syntax.attrs.contains(&always_no_std);
    if contains_always_no_std {
        return CrateSupport::AlwaysNoStd;
    }

    CrateSupport::NotDetected
}

pub struct CheckResult {
    pub package_name: String,
    pub support: CrateSupport,
    pub active_features: Vec<Feature>,
}

impl CheckResult {
    pub fn no_std_itself(&self) -> bool {
        match self.support {
            CrateSupport::AlwaysNoStd => true,
            CrateSupport::ProcMacro => true,
            CrateSupport::OnlyWithoutFeature(ref feature) => !self.is_feature_active(feature),
            CrateSupport::NotDetected => false,
        }
    }

    pub fn is_feature_active(&self, feature: &str) -> bool {
        self.find_active_feature_by_name(feature).is_some()
    }

    pub fn find_active_feature_by_name(&self, feature: &str) -> Option<&Feature> {
        self.active_features.iter().find(|n| &n.name == feature)
    }
}
