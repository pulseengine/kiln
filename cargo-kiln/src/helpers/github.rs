//! GitHub integration utilities for cargo-wrt
//!
//! Provides functions for integrating cargo-wrt reports with GitHub
//! workflows, including PR comments and issue creation.

use std::env;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::formatters::{
    MarkdownFormatter, MarkdownReportGenerator, create_github_pr_comment,
    html::{DocumentationReportData, RequirementData, SafetyReportData},
};

/// GitHub context information from environment variables
#[derive(Debug, Clone)]
pub struct GitHubContext {
    pub token: String,
    pub repository: String,
    pub pr_number: Option<u32>,
    pub sha: Option<String>,
    pub workflow: Option<String>,
}

impl GitHubContext {
    /// Create GitHub context from environment variables (GitHub Actions)
    pub fn from_env() -> Option<Self> {
        let token = env::var("GITHUB_TOKEN").ok()?;
        let repository = env::var("GITHUB_REPOSITORY").ok()?;

        Some(Self {
            token,
            repository,
            pr_number: env::var("GITHUB_PR_NUMBER").ok().and_then(|s| s.parse().ok()),
            sha: env::var("GITHUB_SHA").ok(),
            workflow: env::var("GITHUB_WORKFLOW").ok(),
        })
    }

    /// Check if we're running in a PR context
    pub fn is_pr(&self) -> bool {
        self.pr_number.is_some()
    }
}

/// Post a safety verification report as a GitHub PR comment
pub async fn post_safety_report_comment(
    context: &GitHubContext,
    report: &SafetyReportData,
) -> Result<()> {
    if !context.is_pr() {
        return Err(anyhow::anyhow!("Not running in a PR context"));
    }

    let formatter = MarkdownFormatter::github();
    let report_markdown = MarkdownReportGenerator::safety_report(report, &formatter)?;

    // Create sections for collapsible details
    let mut details = vec![];

    // Add test details if there are failures
    if report.test_summary.failed_tests > 0 {
        details.push((
            "Failed Tests Details",
            "See test output for detailed failure information",
        ));
    }

    // Format recommendations if any
    let recommendations = if !report.recommendations.is_empty() {
        Some(
            report
                .recommendations
                .iter()
                .enumerate()
                .map(|(i, r)| format!("{}. {}", i + 1, r))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    } else {
        None
    };

    // Add recommendations to details if present
    if let Some(ref rec_text) = recommendations {
        details.push(("Recommendations", rec_text));
    }

    // Build comment
    let comment = create_github_pr_comment(
        "🛡️ Safety Verification Report",
        &format!(
            "Overall compliance: **{:.1}%** {}",
            report.overall_compliance,
            get_status_emoji(report.overall_compliance)
        ),
        details,
    );

    post_pr_comment(context, &comment).await
}

/// Post a requirements matrix as a GitHub PR comment
pub async fn post_requirements_comment(
    context: &GitHubContext,
    requirements: &[RequirementData],
) -> Result<()> {
    if !context.is_pr() {
        return Err(anyhow::anyhow!("Not running in a PR context"));
    }

    let formatter = MarkdownFormatter::github();
    let matrix_markdown = MarkdownReportGenerator::requirements_matrix(requirements, &formatter)?;

    // For large matrices, truncate and provide a link
    let comment = if matrix_markdown.len() > 65000 {
        // GitHub comment limit
        create_github_pr_comment(
            "📋 Requirements Traceability Matrix",
            "Matrix too large for comment. See workflow artifacts for full report.",
            vec![],
        )
    } else {
        matrix_markdown
    };

    post_pr_comment(context, &comment).await
}

/// Post documentation compliance report as a GitHub PR comment
pub async fn post_documentation_comment(
    context: &GitHubContext,
    report: &DocumentationReportData,
) -> Result<()> {
    if !context.is_pr() {
        return Err(anyhow::anyhow!("Not running in a PR context"));
    }

    let formatter = MarkdownFormatter::github();
    let report_markdown = MarkdownReportGenerator::documentation_report(report, &formatter)?;

    post_pr_comment(context, &report_markdown).await
}

/// Post a comment to a GitHub PR
async fn post_pr_comment(context: &GitHubContext, comment: &str) -> Result<()> {
    let pr_number = context.pr_number.ok_or_else(|| anyhow::anyhow!("No PR number available"))?;

    let url = format!(
        "https://api.github.com/repos/{}/issues/{}/comments",
        context.repository, pr_number
    );

    #[derive(Serialize)]
    struct CommentPayload {
        body: String,
    }

    let payload = CommentPayload {
        body: comment.to_string(),
    };

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("token {}", context.token))
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "cargo-wrt")
        .json(&payload)
        .send()
        .await
        .context("Failed to post GitHub comment")?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await?;
        return Err(anyhow::anyhow!(
            "GitHub API error: {} - {}",
            status,
            error_text
        ));
    }

    Ok(())
}

/// Get status emoji based on percentage
fn get_status_emoji(percentage: f64) -> &'static str {
    match percentage {
        p if p >= 95.0 => "✅",
        p if p >= 85.0 => "🟢",
        p if p >= 70.0 => "🟡",
        p if p >= 50.0 => "🟠",
        _ => "🔴",
    }
}

/// Generate a GitHub Actions workflow summary
pub fn generate_workflow_summary(
    requirements: Option<&[RequirementData]>,
    safety_report: Option<&SafetyReportData>,
    documentation_report: Option<&DocumentationReportData>,
) -> Result<String> {
    let mut summary = String::new();

    summary.push_str("# WRT Verification Summary\n\n");

    // Requirements summary
    if let Some(reqs) = requirements {
        let total = reqs.len();
        let implemented = reqs.iter().filter(|r| !r.implementations.is_empty()).count();
        let coverage = if total > 0 { (implemented as f64 / total as f64) * 100.0 } else { 0.0 };

        summary.push_str(&format!(
            "## 📋 Requirements\n- Total: {}\n- Coverage: {:.1}%\n\n",
            total, coverage
        ));
    }

    // Safety summary
    if let Some(safety) = safety_report {
        summary.push_str(&format!(
            "## 🛡️ Safety\n- Compliance: {:.1}% {}\n- Tests: {} passed, {} failed\n\n",
            safety.overall_compliance,
            get_status_emoji(safety.overall_compliance),
            safety.test_summary.passed_tests,
            safety.test_summary.failed_tests
        ));
    }

    // Documentation summary
    if let Some(docs) = documentation_report {
        summary.push_str(&format!(
            "## 📚 Documentation\n- Compliance: {:.1}% {}\n- Violations: {} ({} critical)\n\n",
            docs.overall_compliance,
            get_status_emoji(docs.overall_compliance),
            docs.total_violations,
            docs.critical_violations
        ));
    }

    Ok(summary)
}

/// Configuration for GitHub integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubConfig {
    /// Post comments on PRs
    pub post_comments: bool,

    /// Update existing comments instead of creating new ones
    pub update_comments: bool,

    /// Fail workflow if compliance is below threshold
    pub fail_on_low_compliance: bool,

    /// Minimum compliance percentage required
    pub min_compliance: f64,

    /// Create issues for critical violations
    pub create_issues: bool,
}
