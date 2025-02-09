use std::process::ExitCode;

use clap::Parser;

use mago_feedback::create_progress_bar;
use mago_feedback::remove_progress_bar;
use mago_feedback::ProgressBarTheme;
use mago_interner::ThreadedInterner;
use mago_linter::settings::RuleSettings;
use mago_linter::settings::Settings;
use mago_linter::Linter;
use mago_reflection::CodebaseReflection;
use mago_reflector::reflect;
use mago_reporting::reporter::Reporter;
use mago_reporting::reporter::ReportingFormat;
use mago_reporting::reporter::ReportingTarget;
use mago_reporting::Issue;
use mago_reporting::IssueCollection;
use mago_reporting::Level;
use mago_semantics::Semantics;
use mago_source::error::SourceError;
use mago_source::SourceManager;

use crate::config::linter::LinterConfiguration;
use crate::config::linter::LinterLevel;
use crate::config::Configuration;
use crate::enum_variants;
use crate::error::Error;
use crate::reflection::reflect_all_external_sources;
use crate::source;

#[derive(Parser, Debug)]
#[command(
    name = "lint",
    about = "analyze and highlight issues in the project source code using configurable linting rules",
    long_about = r#"
The `lint` command is a powerful tool for analyzing your PHP codebase. By default, it performs
a full analysis, including parsing, semantic checks, and linting based on customizable rules.

This command is ideal for enforcing code quality standards, debugging issues, and maintaining
a consistent, clean codebase. Use `--semantics-only` for a quick validation of code correctness
or the default mode for a comprehensive analysis.
"#
)]
pub struct LintCommand {
    /// Filter the output to only show issues that can be automatically fixed with `mago fix`.
    #[arg(long, short = 'f', help = "filter the output to only show fixable issues", default_value_t = false)]
    pub fixable_only: bool,

    /// Perform only semantic analysis (parsing and semantic checks).
    #[arg(long, short = 's', help = "only perform parsing and semantic checks", default_value_t = false)]
    pub semantics_only: bool,

    /// Specify where the results should be reported.
    #[arg(
        long,
        default_value_t,
        help = "specify where the results should be reported",
        ignore_case = true,
        value_parser = enum_variants!(ReportingTarget)
    )]
    pub reporting_target: ReportingTarget,

    /// Choose the format for reporting issues.
    #[arg(
        long,
        default_value_t,
        help = "choose the format for reporting issues",
        ignore_case = true,
        value_parser = enum_variants!(ReportingFormat)
    )]
    pub reporting_format: ReportingFormat,
}

pub async fn execute(command: LintCommand, configuration: Configuration) -> Result<ExitCode, Error> {
    let interner = ThreadedInterner::new();
    let source_manager = source::load(&interner, &configuration.source, !command.semantics_only).await?;

    let issues = if command.semantics_only {
        check_sources(&interner, &source_manager).await?
    } else {
        lint_sources(&interner, &source_manager, &configuration.linter).await?
    };

    let issues_contain_errors = issues.get_highest_level().is_some_and(|level| level >= Level::Error);

    let reporter = Reporter::new(interner, source_manager, command.reporting_target);

    if command.fixable_only {
        reporter.report(issues.only_fixable(), command.reporting_format)?;
    } else {
        reporter.report(issues, command.reporting_format)?;
    }

    Ok(if issues_contain_errors { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}

pub(super) fn create_linter(
    interner: &ThreadedInterner,
    configuration: &LinterConfiguration,
    codebase: CodebaseReflection,
) -> Linter {
    let mut settings = Settings::new();

    if let Some(level) = configuration.level {
        settings = match level {
            LinterLevel::Off => settings.off(),
            LinterLevel::Help => settings.with_level(Level::Help),
            LinterLevel::Note => settings.with_level(Level::Note),
            LinterLevel::Warning => settings.with_level(Level::Warning),
            LinterLevel::Error => settings.with_level(Level::Error),
        };
    }

    if let Some(default_plugins) = configuration.default_plugins {
        settings = settings.with_default_plugins(default_plugins);
    }

    settings = settings.with_plugins(configuration.plugins.clone());

    for rule in &configuration.rules {
        let rule_settings = match rule.level {
            Some(linter_level) => match linter_level {
                LinterLevel::Off => RuleSettings::disabled(),
                LinterLevel::Help => RuleSettings::from_level(Some(Level::Help)),
                LinterLevel::Note => RuleSettings::from_level(Some(Level::Note)),
                LinterLevel::Warning => RuleSettings::from_level(Some(Level::Warning)),
                LinterLevel::Error => RuleSettings::from_level(Some(Level::Error)),
            },
            None => RuleSettings::enabled(),
        };

        settings = settings.with_rule(rule.name.clone(), rule_settings.with_options(rule.options.clone()));
    }

    let mut linter = Linter::new(settings, interner.clone(), codebase);

    mago_linter::foreach_plugin!(|plugin| {
        linter.add_plugin(plugin);
    });

    linter
}

#[inline]
pub(super) async fn lint_sources(
    interner: &ThreadedInterner,
    manager: &SourceManager,
    configuration: &LinterConfiguration,
) -> Result<IssueCollection, Error> {
    // Collect all user-defined sources.
    let sources: Vec<_> = manager.user_defined_source_ids().collect();
    let length = sources.len();

    let progress_bar = create_progress_bar(length, "🔎  Scanning", ProgressBarTheme::Yellow);
    let mut codebase = reflect_all_external_sources(interner, manager).await?;
    let mut handles = Vec::with_capacity(length);
    for source_id in sources {
        handles.push(tokio::spawn({
            let interner = interner.clone();
            let manager = manager.clone();
            let progress_bar = progress_bar.clone();

            async move {
                // Step 1: load the source
                let source = manager.load(&source_id)?;
                // Step 2: build semantics
                let semantics = Semantics::build(&interner, source);
                let reflections = reflect(&interner, &semantics.source, &semantics.program, &semantics.names);
                progress_bar.inc(1);

                Result::<_, Error>::Ok((semantics, reflections))
            }
        }));
    }

    let mut semantics = Vec::with_capacity(length);
    for handle in handles {
        let (semantic, reflections) = handle.await??;

        codebase = mago_reflector::merge(interner, codebase, reflections);
        semantics.push(semantic);
    }

    mago_reflector::populate(interner, &mut codebase);

    remove_progress_bar(progress_bar);

    let linter = create_linter(interner, configuration, codebase);
    let progress_bar = create_progress_bar(length, "🧹  Linting", ProgressBarTheme::Yellow);
    let mut handles = Vec::with_capacity(length);
    for semantic in semantics {
        handles.push(tokio::spawn({
            let linter = linter.clone();
            let progress_bar = progress_bar.clone();

            async move {
                let mut issues = linter.lint(&semantic);
                issues.extend(semantic.issues);
                if let Some(error) = &semantic.parse_error {
                    issues.push(Into::<Issue>::into(error));
                }

                progress_bar.inc(1);

                Result::<_, SourceError>::Ok(issues)
            }
        }));
    }

    let mut results = Vec::with_capacity(length);
    for handle in handles {
        results.push(handle.await??);
    }

    remove_progress_bar(progress_bar);

    Ok(IssueCollection::from(results.into_iter().flatten()))
}

#[inline]
pub(super) async fn check_sources(
    interner: &ThreadedInterner,
    manager: &SourceManager,
) -> Result<IssueCollection, Error> {
    // Collect all user-defined sources.
    let sources: Vec<_> = manager.user_defined_source_ids().collect();
    let length = sources.len();

    let progress_bar = create_progress_bar(length, "🔎  Scanning", ProgressBarTheme::Yellow);
    let mut handles = Vec::with_capacity(length);
    for source_id in sources {
        handles.push(tokio::spawn({
            let interner = interner.clone();
            let manager = manager.clone();
            let progress_bar = progress_bar.clone();

            async move {
                let source = manager.load(&source_id)?;
                let semantics = Semantics::build(&interner, source);
                progress_bar.inc(1);

                Result::<_, Error>::Ok(semantics)
            }
        }));
    }

    let mut results = Vec::with_capacity(length);
    for handle in handles {
        let semantic = handle.await??;

        if let Some(error) = &semantic.parse_error {
            results.push(Into::<Issue>::into(error));
        }

        results.extend(semantic.issues);
    }

    remove_progress_bar(progress_bar);

    Ok(IssueCollection::from(results.into_iter()))
}
