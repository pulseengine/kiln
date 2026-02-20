//! Enhanced progress indicators and user feedback for cargo-wrt
//!
//! Provides interactive progress bars, spinners, and status indicators
//! to improve user experience during long-running operations.

use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

use colored::Colorize;
use wrt_build_core::formatters::OutputFormat;

/// Progress indicator types
#[derive(Debug, Clone)]
pub enum ProgressStyle {
    /// Simple spinner for indeterminate progress
    Spinner,
    /// Progress bar with percentage for determinate progress
    Bar { current: usize, total: usize },
    /// Step-based progress (e.g., "Step 2 of 5")
    Steps { current: usize, total: usize },
    /// Elapsed time indicator
    Timer,
}

/// Progress indicator configuration
#[derive(Debug, Clone)]
pub struct ProgressConfig {
    pub style: ProgressStyle,
    pub message: String,
    pub show_elapsed: bool,
    pub show_eta: bool,
    pub update_interval: Duration,
    pub use_colors: bool,
}

impl Default for ProgressConfig {
    fn default() -> Self {
        Self {
            style: ProgressStyle::Spinner,
            message: "Processing...".to_string(),
            show_elapsed: true,
            show_eta: false,
            update_interval: Duration::from_millis(100),
            use_colors: true,
        }
    }
}

/// Enhanced progress indicator
pub struct ProgressIndicator {
    config: ProgressConfig,
    start_time: Instant,
    last_update: Instant,
    frame: usize,
    is_active: bool,
    output_format: OutputFormat,
}

impl ProgressIndicator {
    /// Create a new progress indicator
    pub fn new(config: ProgressConfig, output_format: OutputFormat) -> Self {
        let now = Instant::now();
        Self {
            config,
            start_time: now,
            last_update: now,
            frame: 0,
            is_active: false,
            output_format,
        }
    }

    /// Create a spinner progress indicator
    pub fn spinner(
        message: impl Into<String>,
        output_format: OutputFormat,
        use_colors: bool,
    ) -> Self {
        Self::new(
            ProgressConfig {
                style: ProgressStyle::Spinner,
                message: message.into(),
                use_colors,
                ..Default::default()
            },
            output_format,
        )
    }

    /// Create a progress bar
    pub fn bar(
        message: impl Into<String>,
        total: usize,
        output_format: OutputFormat,
        use_colors: bool,
    ) -> Self {
        Self::new(
            ProgressConfig {
                style: ProgressStyle::Bar { current: 0, total },
                message: message.into(),
                use_colors,
                show_eta: true,
                ..Default::default()
            },
            output_format,
        )
    }

    /// Create a step-based indicator
    pub fn steps(
        message: impl Into<String>,
        total: usize,
        output_format: OutputFormat,
        use_colors: bool,
    ) -> Self {
        Self::new(
            ProgressConfig {
                style: ProgressStyle::Steps { current: 0, total },
                message: message.into(),
                use_colors,
                ..Default::default()
            },
            output_format,
        )
    }

    /// Start the progress indicator
    pub fn start(&mut self) {
        if matches!(self.output_format, OutputFormat::Human) {
            self.is_active = true;
            self.render();
        }
    }

    /// Update progress (for determinate progress)
    pub fn update(&mut self, current: usize) {
        if !self.is_active {
            return;
        }

        match &mut self.config.style {
            ProgressStyle::Bar { current: c, .. } => *c = current,
            ProgressStyle::Steps { current: c, .. } => *c = current,
            _ => {},
        }

        let now = Instant::now();
        if now.duration_since(self.last_update) >= self.config.update_interval {
            self.render();
            self.last_update = now;
        }
    }

    /// Advance one step (for step-based progress)
    pub fn advance(&mut self) {
        if let ProgressStyle::Steps { current, .. } = &mut self.config.style {
            *current += 1;
        }
        self.tick();
    }

    /// Tick the progress indicator (for spinners)
    pub fn tick(&mut self) {
        if !self.is_active {
            return;
        }

        self.frame += 1;
        let now = Instant::now();
        if now.duration_since(self.last_update) >= self.config.update_interval {
            self.render();
            self.last_update = now;
        }
    }

    /// Finish the progress indicator with a success message
    pub fn finish_with_message(&mut self, message: impl Into<String>) {
        if !self.is_active {
            return;
        }

        self.clear();
        let elapsed = self.start_time.elapsed();

        if self.config.use_colors {
            println!(
                "{} {} {}",
                "✅".bright_green(),
                message.into().bright_white(),
                format!("({})", format_duration(elapsed)).bright_black()
            );
        } else {
            println!("✅ {} ({})", message.into(), format_duration(elapsed));
        }

        self.is_active = false;
    }

    /// Finish with success
    pub fn finish(&mut self) {
        self.finish_with_message(&self.config.message.clone());
    }

    /// Finish with error
    pub fn finish_with_error(&mut self, error: impl Into<String>) {
        if !self.is_active {
            return;
        }

        self.clear();
        let elapsed = self.start_time.elapsed();

        if self.config.use_colors {
            println!(
                "{} {} {}",
                "❌".bright_red(),
                error.into().bright_red(),
                format!("({})", format_duration(elapsed)).bright_black()
            );
        } else {
            println!("❌ {} ({})", error.into(), format_duration(elapsed));
        }

        self.is_active = false;
    }

    /// Clear the current progress line
    fn clear(&self) {
        if matches!(self.output_format, OutputFormat::Human) {
            print!("\r\x1b[2K");
            io::stdout().flush().unwrap_or(());
        }
    }

    /// Render the current progress state
    fn render(&self) {
        if !matches!(self.output_format, OutputFormat::Human) {
            return;
        }

        self.clear();

        let elapsed = self.start_time.elapsed();
        let mut line = String::new();

        // Add style-specific indicator
        match &self.config.style {
            ProgressStyle::Spinner => {
                let spinner_chars = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let spinner_char = spinner_chars[self.frame % spinner_chars.len()];

                if self.config.use_colors {
                    line.push_str(&format!("{} ", spinner_char.bright_blue()));
                } else {
                    line.push_str(&format!("{} ", spinner_char));
                }
            },
            ProgressStyle::Bar { current, total } => {
                let percentage = (*current as f64 / *total as f64 * 100.0).min(100.0);
                let filled = (percentage / 100.0 * 30.0) as usize;
                let empty = 30 - filled;

                let bar = "█".repeat(filled) + &"░".repeat(empty);

                if self.config.use_colors {
                    line.push_str(&format!("{} {:>3.0}% ", bar.bright_green(), percentage));
                } else {
                    line.push_str(&format!("{} {:>3.0}% ", bar, percentage));
                }
            },
            ProgressStyle::Steps { current, total } => {
                if self.config.use_colors {
                    line.push_str(&format!(
                        "{} Step {}/{} ",
                        "📋".bright_blue(),
                        current.to_string().bright_white(),
                        total.to_string().bright_white()
                    ));
                } else {
                    line.push_str(&format!("📋 Step {}/{} ", current, total));
                }
            },
            ProgressStyle::Timer => {
                if self.config.use_colors {
                    line.push_str(&format!("{} ", "⏱️".bright_blue()));
                } else {
                    line.push_str("⏱️ ");
                }
            },
        }

        // Add message
        if self.config.use_colors {
            line.push_str(&self.config.message.bright_white().to_string());
        } else {
            line.push_str(&self.config.message);
        }

        // Add timing information
        if self.config.show_elapsed {
            if self.config.use_colors {
                line.push_str(&format!(
                    " {}",
                    format!("({})", format_duration(elapsed)).bright_black()
                ));
            } else {
                line.push_str(&format!(" ({})", format_duration(elapsed)));
            }
        }

        // Add ETA if applicable
        if self.config.show_eta {
            if let ProgressStyle::Bar { current, total } = &self.config.style {
                if *current > 0 && *current < *total {
                    let rate = *current as f64 / elapsed.as_secs_f64();
                    let remaining = (*total - *current) as f64 / rate;
                    let eta = Duration::from_secs_f64(remaining);

                    if self.config.use_colors {
                        line.push_str(&format!(
                            " {}",
                            format!("ETA: {}", format_duration(eta)).bright_black()
                        ));
                    } else {
                        line.push_str(&format!(" ETA: {}", format_duration(eta)));
                    }
                }
            }
        }

        print!("{}", line);
        io::stdout().flush().unwrap_or(());
    }
}

impl Drop for ProgressIndicator {
    fn drop(&mut self) {
        if self.is_active {
            self.clear();
        }
    }
}

/// Format a duration for display
fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();

    if total_secs < 60 {
        format!("{:.1}s", duration.as_secs_f64())
    } else if total_secs < 3600 {
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{}m{}s", mins, secs)
    } else {
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        format!("{}h{}m", hours, mins)
    }
}

/// Progress tracking for multi-step operations
pub struct MultiStepProgress {
    steps: Vec<String>,
    current_step: usize,
    step_indicator: ProgressIndicator,
    current_operation: Option<ProgressIndicator>,
}

impl MultiStepProgress {
    /// Create a new multi-step progress tracker
    pub fn new(steps: Vec<String>, output_format: OutputFormat, use_colors: bool) -> Self {
        let total_steps = steps.len();
        let step_indicator = ProgressIndicator::steps(
            "Starting...",
            total_steps,
            output_format.clone(),
            use_colors,
        );

        Self {
            steps,
            current_step: 0,
            step_indicator,
            current_operation: None,
        }
    }

    /// Start the multi-step progress
    pub fn start(&mut self) {
        self.step_indicator.start();
    }

    /// Begin a new step
    pub fn begin_step(&mut self, operation_message: impl Into<String>) {
        if self.current_step < self.steps.len() {
            // Finish previous operation if any
            if let Some(ref mut op) = self.current_operation {
                op.finish();
            }

            let message = operation_message.into();

            // Update step progress
            self.step_indicator.config.message =
                format!("{}: {}", self.steps[self.current_step], message);
            self.step_indicator.advance();

            // Start new operation progress
            self.current_operation = Some(ProgressIndicator::spinner(
                message.clone(),
                self.step_indicator.output_format.clone(),
                self.step_indicator.config.use_colors,
            ));

            if let Some(ref mut op) = self.current_operation {
                op.start();
            }

            self.current_step += 1;
        }
    }

    /// Update current operation progress
    pub fn update_operation(&mut self, message: impl Into<String>) {
        if let Some(ref mut op) = self.current_operation {
            op.config.message = message.into();
            op.tick();
        }
    }

    /// Finish current step with success
    pub fn finish_step(&mut self, message: impl Into<String>) {
        if let Some(ref mut op) = self.current_operation {
            op.finish_with_message(message);
            self.current_operation = None;
        }
    }

    /// Finish current step with error
    pub fn finish_step_with_error(&mut self, error: impl Into<String>) {
        if let Some(ref mut op) = self.current_operation {
            op.finish_with_error(error);
            self.current_operation = None;
        }
    }

    /// Finish all steps
    pub fn finish(&mut self, message: impl Into<String>) {
        if let Some(ref mut op) = self.current_operation {
            op.finish();
        }
        self.step_indicator.finish_with_message(message);
    }

    /// Finish with error
    pub fn finish_with_error(&mut self, error: impl Into<String>) {
        let error_msg = error.into();
        if let Some(ref mut op) = self.current_operation {
            op.finish_with_error(&error_msg);
        }
        self.step_indicator.finish_with_error(error_msg);
    }
}
