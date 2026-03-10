# Security Policy

At TrueFlow, we take the security of your AI infrastructure seriously. We appreciate the efforts of the security community to keep our users safe.

## Supported Versions

Currently, only the `main` branch and the latest released version are officially supported with security updates.

| Version | Supported          |
| ------- | ------------------ |
| v1.x    | :white_check_mark: |
| main    | :white_check_mark: |
| < v1.0  | :x:                |

## Reporting a Vulnerability

If you discover a potential security vulnerability in TrueFlow, we kindly ask that you report it to us privately rather than creating a public issue or posting on social media. 

To report a vulnerability, please email us directly at:
**security@trueflow.ai**

### What to include
Please provide as much information as possible to help us understand and reproduce the issue:
* The version of TrueFlow you are running
* Steps to reproduce the vulnerability (a proof-of-concept is highly appreciated)
* Potential impact and attack vectors
* Any potential mitigation strategies you have identified

### Our Process
1. We will acknowledge receipt of your report within 48 hours.
2. We will investigate the issue and determine its validity and severity.
3. If valid, we will develop a patch and coordinate a release.
4. We will keep you informed of our progress throughout the process.
5. Once resolved, we will publish a security advisory and publicly credit you (if desired).

## Out of Scope
The following issues are generally considered out of scope unless they demonstrate a severe security impact:
* Lack of rate limiting on non-sensitive endpoints
* Self-XSS
* Missing security headers that do not directly lead to an exploit
* Best practice suggestions without a demonstrable exploit
* Vulnerabilities in third-party dependencies that are already known and do not expose TrueFlow directly

Thank you for helping us keep TrueFlow secure!
