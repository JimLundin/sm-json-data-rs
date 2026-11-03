//! `tech.json`, `helpers.json`, and `numerics.json`.

use crate::raw::*;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TechFile {
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    pub tech_categories: Vec<TechCategory>,
    pub next_tech_id: Option<u32>,
}

impl TechFile {
    /// Iterates over every tech, including extension techs at any depth.
    pub fn all_techs(&self) -> impl Iterator<Item = &Tech> {
        fn collect<'a>(tech: &'a Tech, out: &mut Vec<&'a Tech>) {
            out.push(tech);
            for extension in tech.extension_techs.iter().flatten() {
                collect(extension, out);
            }
        }
        let mut techs = Vec::new();
        for category in &self.tech_categories {
            for tech in &category.techs {
                collect(tech, &mut techs);
            }
        }
        techs.into_iter()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TechCategory {
    pub name: String,
    pub description: String,
    pub techs: Vec<Tech>,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tech {
    pub id: Option<TechId>,
    pub name: TechName,
    pub tech_requires: Vec<Requirement>,
    pub other_requires: Vec<Requirement>,
    pub note: Option<Note>,
    pub detail_note: Option<Note>,
    pub dev_note: Option<Note>,
    pub extension_techs: Option<Vec<Tech>>,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HelpersFile {
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    pub helper_categories: Vec<HelperCategory>,
}

impl HelpersFile {
    /// Iterates over every helper in every category.
    pub fn all_helpers(&self) -> impl Iterator<Item = &Helper> {
        self.helper_categories.iter().flat_map(|category| &category.helpers)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelperCategory {
    pub name: String,
    pub description: String,
    pub helpers: Vec<Helper>,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Helper {
    pub name: HelperName,
    pub requires: Vec<Requirement>,
    /// Appears only in Map Rando's fork of the data.
    pub map_rando_override: Option<bool>,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NumericsFile {
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    pub numeric_categories: Vec<NumericCategory>,
}

impl NumericsFile {
    /// Iterates over every numeric parameter in every category.
    pub fn all_numerics(&self) -> impl Iterator<Item = &NumericParameter> {
        self.numeric_categories.iter().flat_map(|category| &category.numerics)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumericCategory {
    pub name: String,
    pub description: String,
    pub numerics: Vec<NumericParameter>,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NumericParameter {
    pub name: NumericName,
    /// The default value, which may refer to other parameters.
    pub value: Numeric,
    pub minimum: Option<u32>,
    pub maximum: Option<u32>,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}
