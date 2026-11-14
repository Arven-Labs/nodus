# Security policy

## Reporting a vulnerability

Please **do not open a public issue** for security problems.

Use GitHub's private reporting instead: go to the repository's **Security** tab and click **Report a vulnerability**. Only the maintainer and you can see the report.

Please include:

- what you found and the impact you expect;
- the commit or version, and your OS (nodus currently targets Arch Linux);
- steps or a minimal example to reproduce it.

## Scope and support

nodus is pre-release: no version has been distributed yet, and only the `main` branch is supported. The threat model is in [`project/SECURITY.md`](../project/SECURITY.md); reports about gaps listed there as known are still welcome.

## What to expect

This is a one-person project, so the policy is best effort: I will try to reply within a few days, but there is no guaranteed response or fix time, and no bug bounty.

Please give me reasonable time to fix an issue before disclosing it publicly. Once fixed, I will credit you in the advisory unless you prefer otherwise.
