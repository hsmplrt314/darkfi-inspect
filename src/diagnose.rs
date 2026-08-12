use serde::Serialize;

// The outcome of a diagnostic check is separate from how confident we are
// in that outcome. Unknown means the check could not be performed or verified.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum CheckState {
    Pass,
    Fail,
    Unknown,
}

// Confidence level for a diagnostic finding — how sure are we that this
// check's verdict is correct? Mirrors the original Diagnose-layer design
// from the project's founding scope, pulled forward here so Inspect's
// checks (and later, Diagnose's own checks) all report through one
// consistent system instead of each command inventing its own strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Confidence {
    /// Directly verified against real on-chain data — no ambiguity.
    Confirmed,
    High,
    Medium,
    Low,
    /// The check couldn't run at all (e.g. couldn't fetch data needed
    /// to compare) — this is NOT "failed", it's "we don't know".
    Unknown,
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            Confidence::Confirmed => "CONFIRMED",
            Confidence::High => "HIGH",
            Confidence::Medium => "MEDIUM",
            Confidence::Low => "LOW",
            Confidence::Unknown => "UNKNOWN",
        };
        write!(f, "{s}")
    }
}

// A single diagnostic observation. State describes the result itself;
// confidence describes how strongly we can support that result.
#[derive(Debug, Serialize)]
pub struct CheckResult {
    pub name: String,
    pub state: CheckState,
    pub confidence: Confidence,
    pub message: String,
}

impl CheckResult {
    pub fn confirmed_pass(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            state: CheckState::Pass,
            confidence: Confidence::Confirmed,
            message: message.to_string(),
        }
    }

    pub fn confirmed_fail(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            state: CheckState::Fail,
            confidence: Confidence::Confirmed,
            message: message.to_string(),
        }
    }

    pub fn unknown(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            state: CheckState::Unknown,
            confidence: Confidence::Unknown,
            message: message.to_string(),
        }
    }

    pub fn new(name: &str, state: CheckState, confidence: Confidence, message: &str) -> Self {
        Self {
            name: name.to_string(),
            state,
            confidence,
            message: message.to_string(),
        }
    }

    // Human-readable one-liner, e.g.
    // "  chain_linkage: OK [CONFIRMED] previous hash matches block 43727"
    pub fn print_human(&self) {
        let verdict = match self.state {
            CheckState::Pass => "OK",
            CheckState::Fail => "MISMATCH",
            CheckState::Unknown => "UNKNOWN",
        };

        println!(
            "  {}: {} [{}] {}",
            self.name, verdict, self.confidence, self.message
        );
    }
}

#[derive(Debug, Serialize)]
pub struct BlockInspection {
    pub height: u32,
    pub hash: String,
    pub previous: String,
    pub txs: usize,
    pub checks: Vec<CheckResult>,
    pub summary: DiagnosticSummary,
}

// A compact summary of what the individual checks found.
// This intentionally describes observations rather than assigning
// an overall "health score" to the inspected block.
#[derive(Debug, Serialize)]
pub struct DiagnosticSummary {
    pub passed: usize,
    pub failed: usize,
    pub unknown: usize,
}

impl DiagnosticSummary {
    // Count the observations without judging their severity.
    // Confidence remains attached to each individual CheckResult.
    pub fn from_checks(checks: &[CheckResult]) -> Self {
        Self {
            passed: checks
                .iter()
                .filter(|check| matches!(check.state, CheckState::Pass))
                .count(),
            failed: checks
                .iter()
                .filter(|check| matches!(check.state, CheckState::Fail))
                .count(),
            unknown: checks
                .iter()
                .filter(|check| matches!(check.state, CheckState::Unknown))
                .count(),
        }
    }
}
