# Security Policy

## Supported Versions

Only the latest release on `main` is actively maintained.

| Version | Supported |
| ------- | --------- |
| latest  | ✅        |
| older   | ❌        |

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Report vulnerabilities privately via [GitHub's private vulnerability reporting](https://github.com/sotsuba/yotsubactf/security/advisories/new).

Include as much detail as possible:

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

You can expect an acknowledgement within **48 hours** and a resolution or status update within **7 days**.

## Scope

Things that are in scope:

- Command injection or privilege escalation via Discord slash commands
- SQL injection via user-supplied input
- Unauthorized access to guild data or user reminders
- Bot token or credentials exposure via logs or API responses

Things that are out of scope:

- Denial of service via Discord rate limits (handled by Twilight)
- Vulnerabilities in upstream dependencies (report to the respective maintainers — Dependabot is configured to track these)
- Issues requiring physical access to the host machine

## Dependency Auditing

This project runs `cargo audit` automatically on every push via the security workflow (`.github/workflows/security.yml`). Dependabot is also configured to open PRs for outdated Cargo and GitHub Actions dependencies.
