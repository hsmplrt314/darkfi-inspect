use serde::Serialize;

// The outcome of a diagnostic check is separate from how confident we are
// in that outcome. Unknown means the check could not be performed or verified.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum CheckState {
    Pass,
    Fail,
    Unknown,
}

// Confidence level for a diagnostic check result — how sure are we that this
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
    Unknown,
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            Confidence::Confirmed => "CONFIRMED",
            Confidence::High => "HIGH",
            Confidence::Medium => "MEDIUM",
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
            CheckState::Pass => "PASS",
            CheckState::Fail => "FAIL",
            CheckState::Unknown => "UNKNOWN",
        };

        println!(
            "  {:<20} [{}] [{}] {}",
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

#[derive(Debug, Serialize)]
pub enum DiagnosticVerdict {
    Healthy,
    Attention,
    Failed,
}

impl DiagnosticVerdict {
    pub fn from_summary(summary: &DiagnosticSummary) -> Self {
        if summary.failed > 0 {
            Self::Failed
        } else if summary.unknown > 0 {
            Self::Attention
        } else {
            Self::Healthy
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Healthy => "HEALTHY",
            Self::Attention => "ATTENTION",
            Self::Failed => "FAILED",
        }
    }
}

/// Turn failed/unknown checks into a useful next place to investigate.
pub fn finding_for(check: &CheckResult) -> Option<String> {
    let area = match check.name.as_str() {
        "chain" => "blockchain/chain state",
        "best_fork" => "consensus/fork state",
        "block_target" => "consensus block target",
        "chain_tip" => "confirmed chain tip",
        "chain_linkage" => "blockchain chain linkage",
        "chain_depth" => "consensus/synchronization",
        "peers" => "P2P connectivity",
        "rpc" => "RPC responsiveness",
        "eventgraph_parents" => "DarkIRC/EventGraph synchronization",
        "eventgraph_rotation" => "DarkIRC/EventGraph rotation history",
        "eventgraph_epoch" => "DarkIRC/EventGraph epoch alignment",
        "eventgraph_current" => "DarkIRC/EventGraph current rotation",
        "eventgraph_genesis" => "DarkIRC/EventGraph genesis identity",
        "eventgraph" => "DarkIRC EventGraph RPC",
        _ => return None,
    };

    match check.state {
        CheckState::Pass => None,
        CheckState::Fail => Some(format!("{}: {}", area, check.message)),
        CheckState::Unknown => Some(format!("{}: {}", area, check.message)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_is_healthy_when_all_checks_pass() {
        let checks = vec![CheckResult::confirmed_pass("test", "everything is fine")];
        let summary = DiagnosticSummary::from_checks(&checks);

        assert!(matches!(
            DiagnosticVerdict::from_summary(&summary),
            DiagnosticVerdict::Healthy
        ));
    }

    #[test]
    fn verdict_is_attention_when_check_is_unknown() {
        let checks = vec![CheckResult::unknown("test", "could not determine state")];
        let summary = DiagnosticSummary::from_checks(&checks);

        assert!(matches!(
            DiagnosticVerdict::from_summary(&summary),
            DiagnosticVerdict::Attention
        ));
    }

    #[test]
    fn verdict_is_failed_when_check_fails() {
        let checks = vec![CheckResult::confirmed_fail("test", "something is wrong")];
        let summary = DiagnosticSummary::from_checks(&checks);

        assert!(matches!(
            DiagnosticVerdict::from_summary(&summary),
            DiagnosticVerdict::Failed
        ));
    }

    #[test]
    fn failed_check_produces_finding() {
        let check = CheckResult::confirmed_fail("peers", "no connected peers");

        let finding = finding_for(&check);

        assert!(finding.is_some());
        assert!(finding.unwrap().contains("P2P connectivity"));
    }

    #[test]
    fn passed_check_produces_no_finding() {
        let check = CheckResult::confirmed_pass("peers", "3 peer(s) connected");

        assert!(finding_for(&check).is_none());
    }

    #[test]
    fn failed_eventgraph_epoch_produces_finding() {
        let check = CheckResult::confirmed_fail(
            "eventgraph_epoch",
            "2 of 24 genesis timestamps are outside the canonical DarkIRC hourly epoch",
        );

        let finding = finding_for(&check);

        assert!(finding.is_some());
        assert!(
            finding
                .unwrap()
                .contains("DarkIRC/EventGraph epoch alignment")
        );
    }

    #[test]
    fn failed_eventgraph_current_produces_finding() {
        let check = CheckResult::confirmed_fail(
            "eventgraph_current",
            "latest genesis is ahead of canonical current rotation",
        );

        let finding = finding_for(&check);

        assert!(finding.is_some());
        assert!(
            finding
                .unwrap()
                .contains("DarkIRC/EventGraph current rotation")
        );
    }

    #[test]
    fn unknown_eventgraph_checks_produce_findings() {
        let epoch = CheckResult::unknown("eventgraph_epoch", "no layer-0 genesis timestamps found");

        let current =
            CheckResult::unknown("eventgraph_current", "no layer-0 genesis timestamps found");

        assert!(finding_for(&epoch).is_some());
        assert!(finding_for(&current).is_some());
    }

    #[test]
    fn failed_eventgraph_genesis_produces_finding() {
        let check = CheckResult::confirmed_fail(
            "eventgraph_genesis",
            "1 of 25 genesis ID(s) do not match the canonical DarkIRC genesis identity",
        );

        let finding = finding_for(&check);

        assert!(finding.is_some());
        assert!(
            finding
                .unwrap()
                .contains("DarkIRC/EventGraph genesis identity")
        );
    }

    #[test]
    fn unknown_eventgraph_genesis_produces_finding() {
        let check = CheckResult::unknown("eventgraph_genesis", "no layer-0 genesis events found");

        let finding = finding_for(&check);

        assert!(finding.is_some());
        assert!(
            finding
                .unwrap()
                .contains("DarkIRC/EventGraph genesis identity")
        );
    }
}
