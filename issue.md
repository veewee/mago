## 📌 What Does This PR Do?

Adds a new linter rule `blank-line-between-import-groups` that enforces blank lines between different types of PHP use statement groups (class, function, const imports).

## 🔍 Context & Motivation

This rule enforces the [PER Coding Style 2.0](https://www.php-fig.org/per/coding-style/#23-use-declarations) recommendation for organizing import statements. It's also a port of php-cs-fixer's `BlankLineBetweenImportGroupsFixer` rule.

From PER-CS 2.0 §2.3:
> Import statements MUST never begin with a leading backslash as they must always be fully qualified.
>
> **There MUST be one blank line after the last import statement of a different type.**

**Bad:**
```php
<?php
use Aaa\Ccc;
use function some\fn_a;
use const SOME_CONST;
```

**Good:**
```php
<?php
use Aaa\Ccc;

use function some\fn_a;

use const SOME_CONST;
```

## 🛠️ Summary of Changes

- **Feature:** Added `consistency/blank-line-between-import-groups` rule with auto-fix support
- **Feature:** Added reusable `use_statements` utilities module (`crates/linter/src/rule/utils/use_statements.rs`) for future import-related rules, providing:
  - `UseStatementType` enum for classifying imports (Class, Function, Const, Mixed)
  - `collect_use_statement_groups()` / `collect_all_use_statement_groups()` for gathering use statements
  - `has_blank_line_between()` for whitespace detection

## 📂 Affected Areas

- [x] Linter
- [ ] Formatter
- [ ] CLI
- [ ] Composer Plugin
- [ ] Dependencies
- [ ] Documentation
- [ ] Other (please specify):

## 🔗 Related Issues or PRs

- Implements [PER Coding Style 2.0 §2.3](https://www.php-fig.org/per/coding-style/#23-use-declarations)
- Port of php-cs-fixer's [`BlankLineBetweenImportGroupsFixer`](https://cs.symfony.com/doc/rules/whitespace/blank_line_between_import_groups.html)

## 📝 Notes for Reviewers

- The rule defaults to `Level::Note` severity
- Mixed imports (`use Foo\{const A, function b}`) are intentionally skipped since they contain multiple types internally
- The auto-fix inserts a newline after the first use statement when groups differ
- 16 test cases cover success/failure scenarios including namespaced code
- The `use_statements` utils module is designed to be reused by future import-related rules I'm planning to port
