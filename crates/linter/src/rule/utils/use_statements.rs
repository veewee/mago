//! Utilities for working with PHP use statements.
//!
//! This module provides common functionality for analyzing and manipulating
//! use statements in PHP code, including:
//!
//! - Import type classification (class, function, const, mixed)
//! - Collecting use statements from programs and namespaces
//! - Checking whitespace between statements

use mago_span::HasSpan;
use mago_syntax::ast::Program;
use mago_syntax::ast::Sequence;
use mago_syntax::ast::Statement;
use mago_syntax::ast::Use;
use mago_syntax::ast::UseItems;

/// Represents the type of a PHP use statement import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum UseStatementType {
    /// A class or namespace import: `use Foo\Bar;`
    Class,
    /// A function import: `use function foo;`
    Function,
    /// A constant import: `use const FOO;`
    Const,
    /// A mixed import containing multiple types: `use Foo\{const A, function b};`
    Mixed,
}

#[allow(dead_code)]
impl UseStatementType {
    /// Determines the import type of a use statement.
    ///
    /// # Examples
    ///
    /// - `use Foo\Bar;` → `Class`
    /// - `use function foo;` → `Function`
    /// - `use const FOO;` → `Const`
    /// - `use Foo\{const A, function b};` → `Mixed`
    #[must_use]
    pub fn from_use_statement(use_stmt: &Use<'_>) -> Self {
        match &use_stmt.items {
            UseItems::Sequence(_) => Self::Class,
            UseItems::TypedSequence(typed) => {
                if typed.r#type.is_function() {
                    Self::Function
                } else {
                    Self::Const
                }
            }
            UseItems::TypedList(typed) => {
                if typed.r#type.is_function() {
                    Self::Function
                } else {
                    Self::Const
                }
            }
            UseItems::MixedList(_) => Self::Mixed,
        }
    }

    /// Returns `true` if this is a class/namespace import.
    #[inline]
    #[must_use]
    pub const fn is_class(&self) -> bool {
        matches!(self, Self::Class)
    }

    /// Returns `true` if this is a function import.
    #[inline]
    #[must_use]
    pub const fn is_function(&self) -> bool {
        matches!(self, Self::Function)
    }

    /// Returns `true` if this is a const import.
    #[inline]
    #[must_use]
    pub const fn is_const(&self) -> bool {
        matches!(self, Self::Const)
    }

    /// Returns `true` if this is a mixed import containing multiple types.
    #[inline]
    #[must_use]
    pub const fn is_mixed(&self) -> bool {
        matches!(self, Self::Mixed)
    }
}

/// Collects consecutive use statements from a sequence of statements.
///
/// Returns a vector of vectors, where each inner vector contains a group of
/// consecutive use statements. Non-use statements act as separators between groups.
///
/// # Example
///
/// ```php
/// use A;
/// use B;
/// $x = 1;  // separator
/// use C;
/// ```
///
/// This would return `[[use A, use B], [use C]]`.
#[must_use]
pub fn collect_use_statement_groups<'a, 'arena>(
    statements: &'a Sequence<'arena, Statement<'arena>>,
) -> Vec<Vec<&'a Use<'arena>>> {
    let mut groups = Vec::new();
    let mut current_group: Vec<&Use<'arena>> = Vec::new();

    for statement in statements {
        if let Statement::Use(use_stmt) = statement {
            current_group.push(use_stmt);
        } else if !current_group.is_empty() {
            groups.push(current_group);
            current_group = Vec::new();
        }
    }

    if !current_group.is_empty() {
        groups.push(current_group);
    }

    groups
}

/// Collects all use statement groups from a program, including those in namespaces.
///
/// This iterates through:
/// - Top-level use statements
/// - Use statements inside namespace blocks (both implicit and braced)
#[must_use]
pub fn collect_all_use_statement_groups<'a, 'arena>(
    program: &'a Program<'arena>,
) -> Vec<Vec<&'a Use<'arena>>> {
    let mut all_groups = Vec::new();

    // Collect from top-level statements
    all_groups.extend(collect_use_statement_groups(&program.statements));

    // Collect from namespace blocks
    for statement in &program.statements {
        if let Statement::Namespace(ns) = statement {
            all_groups.extend(collect_use_statement_groups(ns.statements()));
        }
    }

    all_groups
}

/// Counts the number of newlines between two use statements.
#[must_use]
pub fn count_newlines_between(program: &Program<'_>, first: &Use<'_>, second: &Use<'_>) -> usize {
    let start = first.span().end.offset as usize;
    let end = second.span().start.offset as usize;

    if start >= end || end > program.source_text.len() {
        return 0;
    }

    let between = &program.source_text[start..end];
    between.chars().filter(|&c| c == '\n').count()
}

/// Checks if there is a blank line (two or more newlines) between two use statements.
///
/// This examines the source text between the end of the first statement and the
/// start of the second statement.
#[must_use]
pub fn has_blank_line_between(program: &Program<'_>, first: &Use<'_>, second: &Use<'_>) -> bool {
    count_newlines_between(program, first, second) >= 2
}
