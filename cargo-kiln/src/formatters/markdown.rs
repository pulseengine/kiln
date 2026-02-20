//! Markdown output generation for cargo-wrt reports
//!
//! Provides GitHub-flavored Markdown formatting for requirements matrices,
//! safety reports, and documentation compliance reports. Optimized for
//! posting as GitHub PR comments.

use std::{collections::HashMap, fmt::Write};

use anyhow::Result;
use serde::Serialize;

use crate::formatters::html::{
    DocumentationReportData, RequirementData, SafetyReportData, TestSummaryData,
};

/// Markdown formatter for cargo-wrt reports
#[derive(Clone)]
pub struct MarkdownFormatter {
    github_flavor: bool,
    include_summary: bool,
    collapsible_sections: bool,
}

impl Default for MarkdownFormatter {
    fn default() -> Self {
        Self {
            github_flavor: true,
            include_summary: true,
            collapsible_sections: true,
        }
    }
}

impl MarkdownFormatter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn github() -> Self {
        Self {
            github_flavor: true,
            include_summary: true,
            collapsible_sections: true,
        }
    }

    pub fn standard() -> Self {
        Self {
            github_flavor: false,
            include_summary: true,
            collapsible_sections: false,
        }
    }

    pub fn with_summary(mut self, include: bool) -> Self {
        self.include_summary = include;
        self
    }

    pub fn with_collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible_sections = collapsible;
        self
    }

    /// Format content with optional collapsible section
    fn format_section(&self, title: &str, content: &str, emoji: &str) -> String {
        if self.collapsible_sections && self.github_flavor {
            format!(
                "<details>\n<summary>{} <b>{}</b></summary>\n\n{}\n</details>\n",
                emoji, title, content
            )
        } else {
            format!("## {} {}\n\n{}\n", emoji, title, content)
        }
    }

    /// Format a badge for GitHub
    fn format_badge(&self, label: &str, value: &str, color: &str) -> String {
        if self.github_flavor {
            format!(
                "![{}](https://img.shields.io/badge/{}-{}-{})",
                label,
                label.replace(' ', "%20"),
                value.replace(' ', "%20"),
                color
            )
        } else {
            format!("**{}**: {}", label, value)
        }
    }

    /// Format percentage as colored badge
    fn format_percentage_badge(&self, label: &str, percentage: f64) -> String {
        let color = match percentage {
            p if p >= 95.0 => "brightgreen",
            p if p >= 85.0 => "green",
            p if p >= 70.0 => "yellow",
            p if p >= 50.0 => "orange",
            _ => "red",
        };

        self.format_badge(label, &format!("{:.1}%", percentage), color)
    }
}

/// Markdown report generator for structured data
pub struct MarkdownReportGenerator;

impl MarkdownReportGenerator {
    /// Generate requirements traceability matrix in Markdown
    pub fn requirements_matrix(
        requirements: &[RequirementData],
        formatter: &MarkdownFormatter,
    ) -> Result<String> {
        let mut output = String::new();

        // Header
        writeln!(output, "# 📋 Requirements Traceability Matrix\n")?;

        // Summary section
        if formatter.include_summary {
            let total = requirements.len();
            let implemented = requirements.iter().filter(|r| !r.implementations.is_empty()).count();
            let tested = requirements.iter().filter(|r| !r.tests.is_empty()).count();
            let documented = requirements.iter().filter(|r| !r.documentation.is_empty()).count();

            let coverage =
                if total > 0 { (implemented as f64 / total as f64) * 100.0 } else { 0.0 };

            writeln!(output, "## Summary\n")?;

            if formatter.github_flavor {
                writeln!(
                    output,
                    "{} {} {} {}",
                    formatter.format_badge("Total Requirements", &total.to_string(), "blue"),
                    formatter.format_badge("Implemented", &implemented.to_string(), "green"),
                    formatter.format_badge("Tested", &tested.to_string(), "yellow"),
                    formatter.format_badge("Documented", &documented.to_string(), "purple")
                )?;
                writeln!(output)?;
                writeln!(
                    output,
                    "{}",
                    formatter.format_percentage_badge("Coverage", coverage)
                )?;
            } else {
                writeln!(output, "- **Total Requirements**: {}", total)?;
                writeln!(
                    output,
                    "- **Implemented**: {} ({:.1}%)",
                    implemented,
                    (implemented as f64 / total as f64) * 100.0
                )?;
                writeln!(
                    output,
                    "- **Tested**: {} ({:.1}%)",
                    tested,
                    (tested as f64 / total as f64) * 100.0
                )?;
                writeln!(
                    output,
                    "- **Documented**: {} ({:.1}%)",
                    documented,
                    (documented as f64 / total as f64) * 100.0
                )?;
            }

            writeln!(output)?;
        }

        // Requirements table
        let table_content = format_requirements_table(requirements, formatter)?;
        let section = formatter.format_section("Requirements Details", &table_content, "📊");
        write!(output, "{}", section)?;

        // ASIL breakdown
        let asil_breakdown = generate_asil_breakdown(requirements, formatter)?;
        if !asil_breakdown.is_empty() {
            let section = formatter.format_section("ASIL Level Breakdown", &asil_breakdown, "🛡️");
            write!(output, "\n{}", section)?;
        }

        Ok(output)
    }

    /// Generate safety verification report in Markdown
    pub fn safety_report(
        report: &SafetyReportData,
        formatter: &MarkdownFormatter,
    ) -> Result<String> {
        let mut output = String::new();

        // Header
        writeln!(output, "# 🛡️ Safety Verification Report\n")?;

        // Overall compliance
        writeln!(output, "## Overall Compliance\n")?;

        if formatter.github_flavor {
            writeln!(
                output,
                "{}\n",
                formatter.format_percentage_badge("Overall Compliance", report.overall_compliance)
            )?;
        } else {
            writeln!(
                output,
                "**Overall Compliance**: {:.1}%\n",
                report.overall_compliance
            )?;
        }

        // ASIL compliance table
        if !report.asil_compliance.is_empty() {
            let asil_table = format_asil_compliance_table(&report.asil_compliance, formatter)?;
            let section = formatter.format_section("ASIL Level Compliance", &asil_table, "📊");
            write!(output, "\n{}", section)?;
        }

        // Test summary
        let test_summary = format_test_summary(&report.test_summary, formatter)?;
        let section = formatter.format_section("Test Summary", &test_summary, "🧪");
        write!(output, "\n{}", section)?;

        // Recommendations
        if !report.recommendations.is_empty() {
            let recommendations = format_recommendations(&report.recommendations)?;
            let section = formatter.format_section("Recommendations", &recommendations, "💡");
            write!(output, "\n{}", section)?;
        }

        // GitHub PR comment footer
        if formatter.github_flavor {
            writeln!(output, "\n---")?;
            writeln!(
                output,
                "_Generated by [cargo-wrt](https://github.com/pulseengine/wrt) 🤖_"
            )?;
        }

        Ok(output)
    }

    /// Generate documentation compliance report in Markdown
    pub fn documentation_report(
        report: &DocumentationReportData,
        formatter: &MarkdownFormatter,
    ) -> Result<String> {
        let mut output = String::new();

        // Header
        writeln!(output, "# 📚 Documentation Compliance Report\n")?;

        // Summary
        writeln!(output, "## Summary\n")?;

        if formatter.github_flavor {
            writeln!(
                output,
                "{} {} {} {}",
                formatter.format_percentage_badge("Compliance", report.overall_compliance),
                formatter.format_badge(
                    "Total Requirements",
                    &report.total_requirements.to_string(),
                    "blue"
                ),
                formatter.format_badge(
                    "Violations",
                    &report.total_violations.to_string(),
                    if report.total_violations > 0 { "red" } else { "green" }
                ),
                formatter.format_badge(
                    "Critical",
                    &report.critical_violations.to_string(),
                    if report.critical_violations > 0 { "red" } else { "green" }
                )
            )?;
        } else {
            writeln!(
                output,
                "- **Overall Compliance**: {:.1}%",
                report.overall_compliance
            )?;
            writeln!(
                output,
                "- **Total Requirements**: {}",
                report.total_requirements
            )?;
            writeln!(
                output,
                "- **Total Violations**: {}",
                report.total_violations
            )?;
            writeln!(
                output,
                "- **Critical Violations**: {}",
                report.critical_violations
            )?;
        }

        writeln!(output)?;

        // ASIL compliance
        if !report.asil_compliance.is_empty() {
            let asil_docs = format_asil_documentation(&report.asil_compliance, formatter)?;
            let section = formatter.format_section("Documentation by ASIL Level", &asil_docs, "📊");
            write!(output, "\n{}", section)?;
        }

        Ok(output)
    }
}

// Helper functions for formatting tables and content

fn format_requirements_table(
    requirements: &[RequirementData],
    formatter: &MarkdownFormatter,
) -> Result<String> {
    let mut output = String::new();

    // Table header
    writeln!(
        output,
        "| ID | Title | ASIL | Type | Status | Impl | Tests | Docs |"
    )?;
    writeln!(
        output,
        "|:---|:------|:----:|:-----|:------:|:----:|:-----:|:----:|"
    )?;

    // Table rows
    for req in requirements {
        let impl_icon = if req.implementations.is_empty() { "❌" } else { "✅" };
        let test_icon = if req.tests.is_empty() { "❌" } else { "✅" };
        let doc_icon = if req.documentation.is_empty() { "❌" } else { "✅" };

        let status_badge = if formatter.github_flavor {
            match req.status.as_str() {
                "Verified" => "![Verified](https://img.shields.io/badge/Verified-green)",
                "Implemented" => "![Implemented](https://img.shields.io/badge/Implemented-blue)",
                "Partial" => "![Partial](https://img.shields.io/badge/Partial-yellow)",
                _ => "![Pending](https://img.shields.io/badge/Pending-lightgrey)",
            }
        } else {
            req.status.as_str()
        };

        writeln!(
            output,
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            req.id,
            truncate_string(&req.title, 40),
            req.asil_level,
            req.req_type,
            status_badge,
            impl_icon,
            test_icon,
            doc_icon
        )?;
    }

    Ok(output)
}

fn generate_asil_breakdown(
    requirements: &[RequirementData],
    formatter: &MarkdownFormatter,
) -> Result<String> {
    let mut asil_counts: HashMap<&str, (usize, usize, usize, usize)> = HashMap::new();

    for req in requirements {
        let entry = asil_counts.entry(&req.asil_level).or_insert((0, 0, 0, 0));
        entry.0 += 1; // total
        if !req.implementations.is_empty() {
            entry.1 += 1;
        } // implemented
        if !req.tests.is_empty() {
            entry.2 += 1;
        } // tested
        if !req.documentation.is_empty() {
            entry.3 += 1;
        } // documented
    }

    if asil_counts.is_empty() {
        return Ok(String::new());
    }

    let mut output = String::new();

    writeln!(
        output,
        "| ASIL Level | Total | Implemented | Tested | Documented |"
    )?;
    writeln!(
        output,
        "|:-----------|------:|------------:|-------:|-----------:|"
    )?;

    let mut levels: Vec<_> = asil_counts.keys().collect();
    levels.sort();

    for level in levels {
        let (total, impl_count, test_count, doc_count) = asil_counts[level];
        writeln!(
            output,
            "| {} | {} | {} ({:.0}%) | {} ({:.0}%) | {} ({:.0}%) |",
            level,
            total,
            impl_count,
            (impl_count as f64 / total as f64) * 100.0,
            test_count,
            (test_count as f64 / total as f64) * 100.0,
            doc_count,
            (doc_count as f64 / total as f64) * 100.0
        )?;
    }

    Ok(output)
}

fn format_asil_compliance_table(
    compliance: &HashMap<String, f64>,
    formatter: &MarkdownFormatter,
) -> Result<String> {
    let mut output = String::new();

    writeln!(output, "| ASIL Level | Compliance |")?;
    writeln!(output, "|:-----------|:-----------|")?;

    let mut levels: Vec<_> = compliance.keys().collect();
    levels.sort();

    for level in levels {
        let percentage = compliance[level];
        let badge = if formatter.github_flavor {
            formatter.format_percentage_badge("", percentage)
        } else {
            format!("{:.1}%", percentage)
        };

        writeln!(output, "| {} | {} |", level, badge)?;
    }

    Ok(output)
}

fn format_test_summary(summary: &TestSummaryData, formatter: &MarkdownFormatter) -> Result<String> {
    let mut output = String::new();

    if formatter.github_flavor {
        writeln!(output, "- **Total Tests**: {} tests", summary.total_tests)?;
        writeln!(
            output,
            "- **Results**: {} passed, {} failed",
            summary.passed_tests, summary.failed_tests
        )?;
        writeln!(
            output,
            "- **Coverage**: {:.1}%",
            summary.coverage_percentage
        )?;

        if summary.failed_tests == 0 {
            writeln!(output, "\n✅ All tests passing!")?;
        } else {
            writeln!(output, "\n⚠️ {} tests failing", summary.failed_tests)?;
        }
    } else {
        writeln!(output, "- Total Tests: {}", summary.total_tests)?;
        writeln!(output, "- Passed: {}", summary.passed_tests)?;
        writeln!(output, "- Failed: {}", summary.failed_tests)?;
        writeln!(output, "- Coverage: {:.1}%", summary.coverage_percentage)?;
    }

    Ok(output)
}

fn format_recommendations(recommendations: &[String]) -> Result<String> {
    let mut output = String::new();

    for (i, rec) in recommendations.iter().enumerate() {
        writeln!(output, "{}. {}", i + 1, rec)?;
    }

    Ok(output)
}

fn format_asil_documentation(
    compliance: &HashMap<String, f64>,
    formatter: &MarkdownFormatter,
) -> Result<String> {
    let mut output = String::new();

    writeln!(output, "| ASIL Level | Documentation Compliance |")?;
    writeln!(output, "|:-----------|:------------------------|")?;

    let mut levels: Vec<_> = compliance.keys().collect();
    levels.sort();

    for level in levels {
        let percentage = compliance[level];
        let status = if formatter.github_flavor {
            formatter.format_percentage_badge("", percentage)
        } else {
            format!("{:.1}%", percentage)
        };

        writeln!(output, "| {} | {} |", level, status)?;
    }

    Ok(output)
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Helper to create GitHub PR comment with collapsible sections
pub fn create_github_pr_comment(
    title: &str,
    summary: &str,
    details: Vec<(&str, &str)>, // (section_title, section_content)
) -> String {
    let mut output = String::new();

    // Main title and summary
    output.push_str(&format!("## {}\n\n", title));
    output.push_str(&format!("{}\n\n", summary));

    // Collapsible details sections
    for (section_title, section_content) in details {
        output.push_str(&format!(
            "<details>\n<summary><b>{}</b></summary>\n\n{}\n</details>\n\n",
            section_title, section_content
        ));
    }

    // Footer
    output.push_str("---\n");
    output.push_str(
        "_Generated by [cargo-wrt](https://github.com/pulseengine/wrt) safety verification_ 🤖\n",
    );

    output
}
