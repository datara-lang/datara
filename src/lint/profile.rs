use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LintProfile {
    Student,
    #[default]
    Standard,
    Systems,
}

impl FromStr for LintProfile {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "student" => Ok(LintProfile::Student),
            "standard" => Ok(LintProfile::Standard),
            "systems" => Ok(LintProfile::Systems),
            other => Err(format!(
                "Unknown lint profile '{}'. Supported profiles: student, standard, systems",
                other
            )),
        }
    }
}

impl LintProfile {
    /// Determines if a specific lint rule diagnostic should be suppressed under this profile.
    pub fn should_suppress(&self, code: &str) -> bool {
        match self {
            LintProfile::Systems => {
                // Systems profile silences purely aesthetic style warnings
                code.starts_with("style::non_snake_case")
                    || code.starts_with("style::non_camel_case_types")
                    || code.starts_with("style::prefer_for_loop")
                    || code.starts_with("style::bool_comparison")
            }
            _ => false,
        }
    }

    /// Determines if a specific lint warning should escalate to a fatal error.
    pub fn should_escalate_to_error(&self, code: &str) -> bool {
        match self {
            LintProfile::Student => {
                // Student mode strictly forbids quadratic string building, unused variables,
                // and unnecessary mutable state, escalating them to fatal errors.
                code == "L1401"
                    || code == "style::unused_variable"
                    || code == "perf::unnecessary_mut"
                    || code.starts_with("security")
            }
            _ => false,
        }
    }
}
