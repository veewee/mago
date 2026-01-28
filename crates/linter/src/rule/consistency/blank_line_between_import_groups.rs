use indoc::indoc;
use mago_text_edit::TextEdit;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

use mago_reporting::Annotation;
use mago_reporting::Issue;
use mago_reporting::Level;
use mago_span::HasSpan;
use mago_syntax::ast::Node;
use mago_syntax::ast::NodeKind;
use mago_syntax::ast::Program;
use mago_syntax::ast::Use;

use crate::category::Category;
use crate::context::LintContext;
use crate::requirements::RuleRequirements;
use crate::rule::Config;
use crate::rule::LintRule;
use crate::rule::utils::use_statements::UseStatementType;
use crate::rule::utils::use_statements::collect_all_use_statement_groups;
use crate::rule::utils::use_statements::has_blank_line_between;
use crate::rule_meta::RuleMeta;
use crate::settings::RuleSettings;

#[derive(Debug, Clone)]
pub struct BlankLineBetweenImportGroupsRule {
    meta: &'static RuleMeta,
    cfg: BlankLineBetweenImportGroupsConfig,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct BlankLineBetweenImportGroupsConfig {
    pub level: Level,
}

impl Default for BlankLineBetweenImportGroupsConfig {
    fn default() -> Self {
        Self { level: Level::Note }
    }
}

impl Config for BlankLineBetweenImportGroupsConfig {
    fn level(&self) -> Level {
        self.level
    }
}

impl LintRule for BlankLineBetweenImportGroupsRule {
    type Config = BlankLineBetweenImportGroupsConfig;

    fn meta() -> &'static RuleMeta {
        const META: RuleMeta = RuleMeta {
            name: "Blank Line Between Import Groups",
            code: "blank-line-between-import-groups",
            description: indoc! {"
                Enforces a blank line between different types of import groups.
                Import groups are: class/namespace imports, function imports (`use function`),
                and const imports (`use const`).
            "},
            good_example: indoc! {r#"
                <?php

                use Aaa\Ccc;

                use function some\fn_a;

                use const SOME_CONST;
            "#},
            bad_example: indoc! {r#"
                <?php

                use Aaa\Ccc;
                use function some\fn_a;
                use const SOME_CONST;
            "#},
            category: Category::Consistency,
            requirements: RuleRequirements::None,
        };

        &META
    }

    fn targets() -> &'static [NodeKind] {
        const TARGETS: &[NodeKind] = &[NodeKind::Program];
        TARGETS
    }

    fn build(settings: &RuleSettings<Self::Config>) -> Self {
        Self { meta: Self::meta(), cfg: settings.config }
    }

    fn check<'arena>(&self, ctx: &mut LintContext<'_, 'arena>, node: Node<'_, 'arena>) {
        let Node::Program(program) = node else { return };

        for group in collect_all_use_statement_groups(program) {
            self.check_use_group(ctx, program, &group);
        }
    }
}

impl BlankLineBetweenImportGroupsRule {
    fn check_use_group<'arena>(
        &self,
        ctx: &mut LintContext<'_, 'arena>,
        program: &Program<'arena>,
        use_statements: &[&Use<'arena>],
    ) {
        if use_statements.len() < 2 {
            return;
        }

        for window in use_statements.windows(2) {
            let current = window[0];
            let next = window[1];

            let current_type = UseStatementType::from_use_statement(current);
            let next_type = UseStatementType::from_use_statement(next);

            // Skip mixed imports - they contain multiple types
            if current_type.is_mixed() || next_type.is_mixed() {
                continue;
            }

            if current_type != next_type && !has_blank_line_between(program, current, next) {
                self.report_issue(ctx, current, next);
            }
        }
    }

    fn report_issue(&self, ctx: &mut LintContext<'_, '_>, first: &Use<'_>, second: &Use<'_>) {
        let issue = Issue::new(self.cfg.level(), "Missing blank line between import groups.")
            .with_code(self.meta.code)
            .with_annotation(
                Annotation::primary(second.span()).with_message("This import belongs to a different group."),
            )
            .with_annotation(Annotation::secondary(first.span()).with_message("Previous import group ends here."))
            .with_help("Add a blank line between different import types (class, function, const).");

        ctx.collector.propose(issue, |edits| {
            edits.push(TextEdit::insert(first.span().end.offset, "\n".to_string()));
        });
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::BlankLineBetweenImportGroupsRule;
    use crate::test_lint_failure;
    use crate::test_lint_success;

    // Success cases - no issues expected

    test_lint_success! {
        name = blank_line_between_class_and_function_imports,
        rule = BlankLineBetweenImportGroupsRule,
        code = indoc! {r"
            <?php

            use Aaa\Ccc;

            use function some\fn_a;
        "}
    }

    test_lint_success! {
        name = blank_line_between_class_and_const_imports,
        rule = BlankLineBetweenImportGroupsRule,
        code = indoc! {r"
            <?php

            use Foo\Bar;

            use const SOME_CONST;
        "}
    }

    test_lint_success! {
        name = blank_line_between_function_and_const_imports,
        rule = BlankLineBetweenImportGroupsRule,
        code = indoc! {r"
            <?php

            use function foo\bar;

            use const BAZ;
        "}
    }

    test_lint_success! {
        name = same_type_imports_no_blank_line_needed,
        rule = BlankLineBetweenImportGroupsRule,
        code = indoc! {r"
            <?php

            use Aaa;
            use Bbb;
            use Ccc;
        "}
    }

    test_lint_success! {
        name = all_three_groups_separated,
        rule = BlankLineBetweenImportGroupsRule,
        code = indoc! {r"
            <?php

            use Aaa;
            use Bbb;

            use function foo;
            use function bar;

            use const FOO;
            use const BAR;
        "}
    }

    test_lint_success! {
        name = single_use_statement,
        rule = BlankLineBetweenImportGroupsRule,
        code = indoc! {r"
            <?php

            use Foo\Bar;
        "}
    }

    test_lint_success! {
        name = mixed_import_is_skipped,
        rule = BlankLineBetweenImportGroupsRule,
        code = indoc! {r"
            <?php

            use Foo\{const A, function b};
            use function other;
        "}
    }

    test_lint_success! {
        name = namespaced_imports_separated,
        rule = BlankLineBetweenImportGroupsRule,
        code = indoc! {r"
            <?php

            namespace App;

            use Foo\Bar;

            use function baz;
        "}
    }

    test_lint_success! {
        name = braced_namespace_imports_separated,
        rule = BlankLineBetweenImportGroupsRule,
        code = indoc! {r"
            <?php

            namespace App {
                use Foo\Bar;

                use function baz;
            }
        "}
    }

    test_lint_success! {
        name = comment_between_groups_with_blank_line,
        rule = BlankLineBetweenImportGroupsRule,
        code = indoc! {r"
            <?php

            use Aaa;

            // Function imports
            use function foo;
        "}
    }

    // Failure cases - issues expected

    test_lint_failure! {
        name = missing_blank_line_between_class_and_function,
        rule = BlankLineBetweenImportGroupsRule,
        code = indoc! {r"
            <?php

            use Aaa\Ccc;
            use function some\fn_a;
        "}
    }

    test_lint_failure! {
        name = missing_blank_line_between_class_and_const,
        rule = BlankLineBetweenImportGroupsRule,
        code = indoc! {r"
            <?php

            use Foo\Bar;
            use const SOME_CONST;
        "}
    }

    test_lint_failure! {
        name = missing_blank_line_between_function_and_const,
        rule = BlankLineBetweenImportGroupsRule,
        code = indoc! {r"
            <?php

            use function foo;
            use const BAR;
        "}
    }

    test_lint_failure! {
        name = multiple_missing_blank_lines,
        rule = BlankLineBetweenImportGroupsRule,
        count = 2,
        code = indoc! {r"
            <?php

            use Aaa;
            use function foo;
            use const BAR;
        "}
    }

    test_lint_failure! {
        name = missing_blank_line_in_namespace,
        rule = BlankLineBetweenImportGroupsRule,
        code = indoc! {r"
            <?php

            namespace App;

            use Foo\Bar;
            use function baz;
        "}
    }

    test_lint_failure! {
        name = missing_blank_line_in_braced_namespace,
        rule = BlankLineBetweenImportGroupsRule,
        code = indoc! {r"
            <?php

            namespace App {
                use Foo\Bar;
                use function baz;
            }
        "}
    }
}
