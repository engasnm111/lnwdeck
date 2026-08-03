# GOAL.md — lnwdeck Master Goal

## Mission

Implement the **entire lnwdeck project** exactly as specified.

The implementation plan is the **single source of truth**.

---

## Documents to read before any action

Read completely, in this order:

1. AGENTS.md
2. README.md
3. docs/lnwdeck-complete-plan.md
4. docs/superpowers/plans/2026-08-03-lnwdeck-v0.1-implementation.md

Do not start coding until all documents are read.

---

## Global rules

- Work Inline only.
- Never create Subagents unless explicitly requested.
- Never modify the implementation plan.
- Follow TDD.
- Never weaken lint/typecheck/tests.
- Never leave TODO or placeholder implementations.
- Never guess APIs or provider behavior.
- Verify against official documentation when needed.
- Preserve privacy rules at all times.

---

## Execution Strategy

Execute sequentially:

Phase 0

↓

Task 1

↓

Review

↓

Commit

↓

Task 2

↓

Review

↓

Commit

↓

...

↓

Task 20

↓

Final Release Audit

---

## Task Rules

For every task:

1. Read the corresponding Task section from the implementation plan.
2. Perform only that task.
3. Run every required verification command.
4. Fix all failures before continuing.
5. Commit once the task passes.
6. Produce a handoff summary.
7. Stop and wait for approval.

Never start the next task automatically.

---

## Reviews

After every phase:

- Run the review process.
- Fix all Critical and High findings.
- Re-run tests.
- Continue only after approval.

---

## Release Rules

Before release:

- Run Final Release Audit.
- Ensure every acceptance criterion passes.
- Ensure working tree is clean.
- Ensure all documentation matches the implementation.

Do not publish until the final verdict is:

READY TO RELEASE

---

## Success Criteria

The project is complete only when:

- All 20 tasks are finished.
- All reviews pass.
- All quality gates pass.
- Privacy rules are satisfied.
- Windows x64, ARM64 and x86 builds succeed.
- Documentation is complete.
- Release audit returns READY TO RELEASE.
