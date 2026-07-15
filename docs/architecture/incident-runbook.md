# Incident Runbook

This document defines the process for handling security vulnerabilities in eggfetch, from initial report through public disclosure.

## Vulnerability Response Process

### 1. Report Received

- Reports arrive via private email (dbowman91@proton.me) or GitHub Security Advisory.
- Do not discuss vulnerability details in public issues, PRs, or discussions.
- If the report arrives via a public issue, immediately close it and redirect the reporter to private communication.

### 2. Acknowledge Within 48 Hours

- Send a private acknowledgment to the reporter within 48 hours of receipt.
- Include a tracking identifier (e.g., `EF-YYYY-NNN`).
- Confirm the preferred communication channel for follow-up.
- Do not disclose details to anyone outside the core team during this phase.

### 3. Assess Severity

Classify the vulnerability using the following severity levels:

| Severity | Criteria | Response Time |
|----------|----------|---------------|
| **Critical** | Remote code execution, credential exfiltration, TLS bypass affecting all users, arbitrary memory read/write | Fix within 7 days |
| **High** | Credential leakage to third parties, SSRF to internal networks, decompression bomb with no workaround, proxy credential forwarding | Fix within 14 days |
| **Medium** | Information disclosure in debug output (non-credential), redirect to unintended origins without credential stripping, bypass of single security control | Fix within 30 days |
| **Low** | Theoretical issues with no known exploit, minor information leakage, denial of service with easy workaround | Fix within 60 days |

When in doubt, classify one level higher than the initial assessment.

### 4. Develop Fix on Private Branch

- Create a private branch for the fix (e.g., `security/EF-YYYY-NNN`).
- Do not push the branch to the public remote until the fix is ready for release.
- Write the fix with test coverage for the specific vulnerability.
- Run the full test suite, including the release security checklist items relevant to the fix.
- Do not include unrelated changes in the security fix branch.

### 5. Request CVE/GHSA if Applicable

- Request a CVE identifier via GitHub Security Advisory for any vulnerability with a CVSS score of 4.0 or higher.
- Include the following in the advisory draft:
  - Affected versions (introduced version, fixed version)
  - Severity and CVSS score
  - Impact description (what an attacker can achieve)
  - Mitigation steps (if a fix is not immediately available)
  - Credit to the reporter (with permission)

### 6. Coordinate Disclosure with Reporter

- Share the fix and advisory draft with the reporter for review.
- Agree on a disclosure date (see Embargo Policy below).
- The reporter may independently disclose after the embargo deadline.
- Credit the reporter in the advisory unless they prefer anonymity.

### 7. Release Fix

- Merge the fix to main after review and CI passes.
- Bump the version in `Cargo.toml` files.
- Update the changelog with the security fix.
- Publish the release (crate publish, PyPI upload, GitHub release).
- Ensure the release security checklist is completed.

### 8. Publish Advisory

- Publish the GitHub Security Advisory on or after the agreed disclosure date.
- Include the CVE identifier if one was requested.
- Update `SECURITY.md` with the new supported version if applicable.
- Notify downstream consumers (if any) via the advisory.

## Embargo Policy

### Disclosure Deadlines

- **Critical/High**: 90-day disclosure deadline from the date of the initial report.
- **Medium/Low**: 90-day disclosure deadline from the date of the initial report.

### Fix Timing

- Release the fix before the deadline when possible.
- If the fix requires more time, negotiate an extension with the reporter.
- If the fix is not ready by the deadline, the reporter may disclose independently.

### Reporter Advance Notice

- The reporter receives 7-day advance notice before the public disclosure.
- The advance notice includes the advisory draft, fix details, and planned disclosure date.
- The reporter may share the advance notice with downstream consumers under NDA.

### Exception Handling

- If active exploitation is detected,缩短 the embargo period and accelerate the fix release.
- If the vulnerability is independently discovered by a third party,缩短 the embargo period.
- If the vulnerability affects multiple projects, coordinate disclosure with other maintainers.

## CVE/GHSA Process

### Requesting a CVE

1. Create a draft security advisory on GitHub (Settings > Security advisories).
2. Fill in the advisory template:
   - **Title**: Brief description of the vulnerability
   - **Affected versions**: Version range (e.g., `>= 0.1.0, < 0.2.0`)
   - **Fixed version**: Version where the fix is released
   - **Severity**: Critical, High, Medium, or Low
   - **CVSS score**: Calculated using the CVSS v3.1 calculator
   - **Weakness**: CWE identifier (e.g., CWE-200 for information exposure)
   - **Impact**: Description of what an attacker can achieve
   - **Mitigation**: Steps to mitigate the vulnerability before upgrading
3. Submit the advisory for CVE request via GitHub's CVE Numbering Authority (CNA) process.
4. GitHub will assign a CVE identifier (e.g., CVE-YYYY-NNNNN).

### Publishing the Advisory

1. After the fix is released and the embargo period has elapsed, publish the advisory.
2. Include:
   - CVE identifier
   - Affected and fixed versions
   - Severity and CVSS score
   - Detailed impact description
   - Reproduction steps (if applicable)
   - Fix details
   - Credit to reporters
3. The advisory is automatically published on the GitHub Security Advisories page.

### GHSA Process

- GitHub Security Advisories (GHSA) are the primary disclosure mechanism.
- CVEs are requested through GitHub's CNA for external tracking.
- Both GHSA and CVE are published simultaneously on the disclosure date.

## Incident Response Steps

### Phase 1: Confirm Vulnerability

1. Reproduce the reported vulnerability with the provided steps.
2. Determine the affected code path and subsystem.
3. Identify all affected versions (introduction version).
4. Assess whether the vulnerability is actively exploited.

### Phase 2: Assess Impact

1. Determine the worst-case impact (confidentiality, integrity, availability).
2. Identify affected user populations (all users, specific configurations, specific feature usage).
3. Calculate CVSS score using the CVSS v3.1 calculator.
4. Classify severity (Critical, High, Medium, Low).

### Phase 3: Develop Fix

1. Create a private branch for the fix.
2. Write a minimal fix that addresses the vulnerability without introducing regressions.
3. Add test coverage for the specific vulnerability.
4. Run the full test suite: `cargo test --workspace --all-features`.
5. Run the release security checklist items relevant to the fix.

### Phase 4: Test Fix

1. Verify the fix resolves the reported vulnerability.
2. Run the full test suite to ensure no regressions.
3. Run fuzz targets relevant to the affected subsystem.
4. Review the fix for unintended side effects.
5. If applicable, test across the Python bindings and CLI.

### Phase 5: Release Patched Version

1. Merge the fix to main after code review.
2. Bump the version in `Cargo.toml` files.
3. Update the changelog with the security fix.
4. Complete the release security checklist.
5. Publish the release (crate publish, PyPI upload, GitHub release).
6. Tag the release with the version number.

### Phase 6: Publish Security Advisory

1. Publish the GitHub Security Advisory on the agreed disclosure date.
2. Include the CVE identifier, affected versions, fixed version, severity, and impact.
3. Update `SECURITY.md` with the new supported version.
4. Notify downstream consumers via the advisory.

### Phase 7: Notify Affected Users

1. If the vulnerability affects a specific set of users (e.g., users of a specific feature), notify them directly.
2. Post a brief announcement on the project's GitHub Discussions or release notes.
3. Do not disclose vulnerability details beyond what is in the advisory.

## Post-Incident

### Retrospective

After the incident is resolved, conduct a retrospective:

1. What was the root cause?
2. How was the vulnerability introduced?
3. Why was it not caught by existing tests or reviews?
4. What process changes would prevent similar vulnerabilities?
5. Should new fuzz targets, property tests, or CI checks be added?

### Process Updates

- Update this runbook if the response process needs improvement.
- Add new test cases to prevent regression.
- Update the threat model if the attack surface has changed.
- Add new fuzz targets if the affected subsystem was not covered.

## Contact

- **Security reports**: dbowman91@proton.me
- **PGP**: Available on request
- **Response time**: Acknowledgment within 48 hours
- **Disclosure**: Via GitHub Security Advisory
