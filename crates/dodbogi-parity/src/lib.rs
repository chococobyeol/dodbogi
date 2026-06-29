//! Parity scenario and evidence helpers.
//!
//! This crate deliberately stores evidence metadata only.  It must not embed
//! Magpie source-derived text, shader code, assets, or file structure.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScenarioResult {
    Pending,
    Pass,
    Partial,
    Fail,
    Blocked,
    Unsupported,
}

impl ScenarioResult {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Pass => "pass",
            Self::Partial => "partial",
            Self::Fail => "fail",
            Self::Blocked => "blocked",
            Self::Unsupported => "unsupported",
        }
    }

    pub fn is_classified(self) -> bool {
        !matches!(self, Self::Pending)
    }
}

impl fmt::Display for ScenarioResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioId(pub String);

impl ScenarioId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioEvidenceRowInput {
    pub scenario_id: String,
    pub priority: &'static str,
    pub feature_area: &'static str,
    pub magpie_settings: String,
    pub environment: String,
    pub magpie_observation: String,
    pub magpie_artifacts: String,
    pub dodbogi_result: ScenarioResult,
    pub dodbogi_artifacts: String,
    pub tolerance: &'static str,
    pub owner: &'static str,
    pub verifier_notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioEvidenceRow {
    pub scenario_id: ScenarioId,
    pub priority: &'static str,
    pub feature_area: &'static str,
    pub magpie_settings: String,
    pub environment: String,
    pub magpie_observation: String,
    pub magpie_artifacts: String,
    pub dodbogi_result: ScenarioResult,
    pub dodbogi_artifacts: String,
    pub tolerance: &'static str,
    pub owner: &'static str,
    pub verifier_notes: String,
}

impl ScenarioEvidenceRow {
    pub fn new(input: ScenarioEvidenceRowInput) -> Self {
        Self {
            scenario_id: ScenarioId::new(input.scenario_id),
            priority: input.priority,
            feature_area: input.feature_area,
            magpie_settings: input.magpie_settings,
            environment: input.environment,
            magpie_observation: input.magpie_observation,
            magpie_artifacts: input.magpie_artifacts,
            dodbogi_result: input.dodbogi_result,
            dodbogi_artifacts: input.dodbogi_artifacts,
            tolerance: input.tolerance,
            owner: input.owner,
            verifier_notes: input.verifier_notes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScenarioSummary {
    pub total: usize,
    pub classified: usize,
    pub pass: usize,
    pub partial: usize,
    pub blocked: usize,
    pub unsupported: usize,
    pub fail: usize,
    pub pending: usize,
}

impl ScenarioSummary {
    pub fn all_classified(self) -> bool {
        self.total > 0 && self.pending == 0 && self.classified == self.total
    }

    pub fn unapproved_release_blockers(self) -> usize {
        self.partial + self.unsupported + self.blocked + self.fail + self.pending
    }

    pub fn release_clean_without_exceptions(self) -> bool {
        self.total > 0 && self.pass == self.total && self.unapproved_release_blockers() == 0
    }
}

pub fn summarize(rows: &[ScenarioEvidenceRow]) -> ScenarioSummary {
    let mut summary = ScenarioSummary {
        total: rows.len(),
        classified: 0,
        pass: 0,
        partial: 0,
        blocked: 0,
        unsupported: 0,
        fail: 0,
        pending: 0,
    };

    for row in rows {
        if row.dodbogi_result.is_classified() {
            summary.classified += 1;
        }
        match row.dodbogi_result {
            ScenarioResult::Pending => summary.pending += 1,
            ScenarioResult::Pass => summary.pass += 1,
            ScenarioResult::Partial => summary.partial += 1,
            ScenarioResult::Fail => summary.fail += 1,
            ScenarioResult::Blocked => summary.blocked += 1,
            ScenarioResult::Unsupported => summary.unsupported += 1,
        }
    }

    summary
}

pub fn render_markdown(title: &str, rows: &[ScenarioEvidenceRow]) -> String {
    let summary = summarize(rows);
    let mut output = String::new();
    output.push_str("# ");
    output.push_str(title);
    output.push_str("\n\n");
    output.push_str("This matrix is generated from clean-room runtime evidence. It preserves the full scenario-evidence schema and separates evidence classification from release acceptance. A pass from a classification smoke is not a full parity release approval while partial, fail, blocked, pending, or unapproved unsupported rows remain.\n\n");
    output.push_str("## Summary\n\n");
    output.push_str(&format!(
        "- total: {}\n- classified: {}\n- pass: {}\n- partial: {}\n- blocked: {}\n- unsupported: {}\n- fail: {}\n- pending: {}\n- unapproved_release_blockers: {}\n\n",
        summary.total,
        summary.classified,
        summary.pass,
        summary.partial,
        summary.blocked,
        summary.unsupported,
        summary.fail,
        summary.pending,
        summary.unapproved_release_blockers()
    ));
    output.push_str("| scenario_id | priority | feature_area | magpie_settings | environment | magpie_observation | magpie_artifacts | dodbogi_result | dodbogi_artifacts | tolerance | owner | verifier_notes |\n");
    output.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");

    for row in rows {
        output.push_str("| ");
        output.push_str(&escape_cell(&row.scenario_id.0));
        output.push_str(" | ");
        output.push_str(row.priority);
        output.push_str(" | ");
        output.push_str(&escape_cell(row.feature_area));
        output.push_str(" | ");
        output.push_str(&escape_cell(&row.magpie_settings));
        output.push_str(" | ");
        output.push_str(&escape_cell(&row.environment));
        output.push_str(" | ");
        output.push_str(&escape_cell(&row.magpie_observation));
        output.push_str(" | ");
        output.push_str(&escape_cell(&row.magpie_artifacts));
        output.push_str(" | ");
        output.push_str(row.dodbogi_result.as_str());
        output.push_str(" | ");
        output.push_str(&escape_cell(&row.dodbogi_artifacts));
        output.push_str(" | ");
        output.push_str(row.tolerance);
        output.push_str(" | ");
        output.push_str(&escape_cell(row.owner));
        output.push_str(" | ");
        output.push_str(&escape_cell(&row.verifier_notes));
        output.push_str(" |\n");
    }

    output
}

fn escape_cell(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\n', "<br>")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence_row(scenario_id: &str, result: ScenarioResult) -> ScenarioEvidenceRow {
        ScenarioEvidenceRow::new(ScenarioEvidenceRowInput {
            scenario_id: scenario_id.to_string(),
            priority: "P0",
            feature_area: "launch",
            magpie_settings: "reference settings".to_string(),
            environment: "reference environment".to_string(),
            magpie_observation: "tests/reference/magpie-behavior.md#launch".to_string(),
            magpie_artifacts: ".omx/evidence/stage-a/magpie-smoke-run.json".to_string(),
            dodbogi_result: result,
            dodbogi_artifacts: "artifact".to_string(),
            tolerance: "exact",
            owner: "test",
            verifier_notes: "ok".to_string(),
        })
    }

    #[test]
    fn summarize_counts_classified_rows() {
        let rows = vec![
            evidence_row("P0-ONE", ScenarioResult::Pass),
            evidence_row("P3-TWO", ScenarioResult::Unsupported),
        ];

        let summary = summarize(&rows);
        assert_eq!(summary.total, 2);
        assert_eq!(summary.classified, 2);
        assert_eq!(summary.pass, 1);
        assert_eq!(summary.unsupported, 1);
        assert!(summary.all_classified());
        assert!(!summary.release_clean_without_exceptions());
        assert_eq!(summary.unapproved_release_blockers(), 1);
    }

    #[test]
    fn markdown_escapes_cells_and_preserves_full_schema() {
        let rows = vec![ScenarioEvidenceRow::new(ScenarioEvidenceRowInput {
            scenario_id: "P0|ONE".to_string(),
            priority: "P0",
            feature_area: "launch",
            magpie_settings: "settings".to_string(),
            environment: "env".to_string(),
            magpie_observation: "obs".to_string(),
            magpie_artifacts: "ref-artifact".to_string(),
            dodbogi_result: ScenarioResult::Partial,
            dodbogi_artifacts: "artifact".to_string(),
            tolerance: "exact",
            owner: "owner",
            verifier_notes: "line\nbreak".to_string(),
        })];

        let markdown = render_markdown("Matrix", &rows);
        assert!(markdown.contains("magpie_settings"));
        assert!(markdown.contains("magpie_observation"));
        assert!(markdown.contains("owner"));
        assert!(markdown.contains("P0\\|ONE"));
        assert!(markdown.contains("line<br>break"));
        assert!(markdown.contains("partial"));
    }

    #[test]
    fn unsupported_result_uses_schema_value_not_tolerance_value() {
        assert_eq!(ScenarioResult::Unsupported.as_str(), "unsupported");
    }
}
