//! Output formatting for specs

use anyhow::{bail, Result};
use colored::*;

use crate::spec::Spec;

/// Output a full spec
pub fn output_spec(spec: &Spec, json: bool) -> Result<()> {
    if json {
        let json_str = serde_json::to_string_pretty(spec)?;
        println!("{}", json_str);
    } else {
        // Pretty print as YAML
        let yaml_str = serde_yaml::to_string(spec)?;
        println!("{}", yaml_str);
    }
    Ok(())
}

/// Output a specific section of a spec
pub fn output_spec_section(spec: &Spec, section: &str, json: bool) -> Result<()> {
    let value = spec.get_section(section);

    match value {
        Some(v) => {
            if json {
                // Convert YAML value to JSON
                let json_value = yaml_to_json(&v)?;
                println!("{}", serde_json::to_string_pretty(&json_value)?);
            } else {
                println!("{}", format!("# {}", section).bold());
                match v {
                    serde_yaml::Value::String(s) => println!("{}", s),
                    _ => println!("{}", serde_yaml::to_string(&v)?),
                }
            }
            Ok(())
        }
        None => {
            bail!("Section not found: {}", section);
        }
    }
}

/// Convert YAML value to JSON value
fn yaml_to_json(yaml: &serde_yaml::Value) -> Result<serde_json::Value> {
    let yaml_str = serde_yaml::to_string(yaml)?;
    let json_value: serde_json::Value = serde_yaml::from_str(&yaml_str)?;
    Ok(json_value)
}
