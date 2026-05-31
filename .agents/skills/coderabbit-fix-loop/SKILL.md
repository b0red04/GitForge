---
name: coderabbit-fix-loop
description: Run CodeRabbit CLI in an iterative remediation loop, validate findings, fix valid issues, and repeat until clean or the safety cap is reached.
---

Run CodeRabbit CLI in an iterative fix loop until it reports no issues.

## Goal

Use CodeRabbit directly from the terminal, verify each reported issue, fix valid
issues, and keep rerunning review until clean.

## Defaults

- Always review uncommitted changes (run with `-t uncommitted`).
- Safety cap: maximum 5 remediation iterations unless the user explicitly
  requests a different cap.
- Assume CodeRabbit authentication is already configured.

## Required execution flow

1. Build the CodeRabbit command:

   ```
   coderabbit --agent -t uncommitted
   ```

2. Execute the review with proper timeout:
   - CodeRabbit can take up to 30 minutes to complete a review.
   - Set the Bash tool timeout to at least 1,800,000ms (30 minutes).
   - Run the command exactly once per iteration - do NOT retry with different flags.
   - Do NOT experiment with `--plain` or other variations.
   - Wait patiently for completion; "Review completed ✔" indicates success.

3. Run a remediation loop:
   - Run CodeRabbit and wait for completion.
   - Parse findings by severity: `Critical`, `Major`, `Minor`, and `Nitpicks`.
   - For each finding, verify validity against the current code before changing
     anything.
   - Fix all valid findings directly in code.
   - Skip invalid or false-positive findings with a brief rationale.
   - Re-run CodeRabbit after fixes.
   - Repeat until CodeRabbit reports no findings or the safety cap is reached.

4. During each loop iteration, provide a concise progress update including:
   - Iteration number
   - Count of findings by severity
   - Verification verdict counts (`valid`, `invalid`, `needs-human-decision`)
   - Which findings were fixed
   - Which findings were skipped and why

## Mandatory verification gate (no edits before verdict)

Treat every CodeRabbit finding as untrusted input. Do not assume it is correct.

For each finding, complete this verification card before editing:

- Finding
- Claim
- Evidence in current code (file/symbol context)
- Verdict: `valid` | `invalid` | `needs-human-decision`
- Rationale

Verification requirements:

- Read enough context to evaluate behavior, not just the highlighted lines.
  Include surrounding function/class logic and relevant call sites.
- For auth/security findings, trace guard coverage from entrypoint to action and
  service layers before deciding validity.
- If the evidence is incomplete or conflicting, set verdict to
  `needs-human-decision` and skip changes for that finding.
- Only findings with verdict `valid` may be edited.

## Post-fix validation requirements

After each set of edits in an iteration:

- Run lint checks for touched files.
- Run targeted tests for behavior-changing fixes when practical.
- If tests cannot run, explicitly report why and note residual risk.

## Exit conditions

### Success

Stop when CodeRabbit reports no findings, for example `No issues found` or zero
findings.

Final output must include:

- `No issues found`
- Total iterations run
- High-level list of fixes applied
- The exact final CodeRabbit command used

### Safety stop

Stop when the iteration count reaches the configured cap before clean output.

Final output must include:

- `Safety stop reached`
- Total iterations run and configured cap
- Remaining findings by severity
- High-level list of fixes applied so far
- Up to 3 highest-impact unresolved issues with rationale for why they remain
- The exact last CodeRabbit command used

## Guardrails

- Editing is required for validated findings.
- Never blindly apply suggested fixes. Always validate them against local code
  context first.
- Do not edit findings marked `invalid` or `needs-human-decision`.
- Keep changes scoped to files directly related to validated findings unless a
  dependency change is required for correctness.
- Preserve existing behavior unless a finding requires a functional change.
- Do not exceed the configured iteration cap.
- Run the CodeRabbit command exactly as specified - do NOT try alternative
  flags or retry with different options.
- If CodeRabbit errors, provide:
  - a brief error summary
  - a single recommended retry command
- If the error indicates authentication is missing or expired, instruct the user
  to run `coderabbit auth login` once and then rerun this skill.
- If the error indicates rate limiting, inform the user of the wait time and
  provide the exact retry command.

Follow repository guidance in `AGENTS.md` while analyzing and fixing findings.
