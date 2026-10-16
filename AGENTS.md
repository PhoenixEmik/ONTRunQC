# Repository Guidelines

## Project Structure & Module Organization

This directory currently contains no application code, tests, assets, or build configuration. Establish the project layout when adding the first implementation, and update this guide to reflect it. Keep source code, tests, and static assets in clearly named directories appropriate to the chosen framework. For example, use `src/`, `tests/`, and `assets/` if the framework does not prescribe another layout.

## Build, Test, and Development Commands

No build, test, or local development commands are configured yet. When introducing a toolchain, document its prerequisites and exact setup, run, build, and test commands in `README.md`. Prefer repository-defined scripts so contributors can reproduce the same workflow. Do not assume commands such as `npm test` or `make build` exist until their configuration is committed.

## Coding Style & Naming Conventions

Follow the chosen language’s standard conventions and add its formatter or linter configuration alongside the first source files. Match existing formatting once established. Use descriptive names, keep modules focused, and avoid unrelated formatting changes. Document any nonstandard indentation or naming rules here.

## Testing Guidelines

No test framework or coverage threshold is configured. Introduce tests with the first testable behavior, covering expected results and meaningful failure cases. Use descriptive test names and the framework’s normal file naming convention. Document how to run the full suite and individual tests.

## Commit & Pull Request Guidelines

No Git history is available to establish commit conventions. Use concise, imperative commit subjects, such as `Add initial project configuration`, and keep commits focused. Pull requests should explain the purpose, summarize changes, and describe validation performed. Link relevant issues and include screenshots for visible interface changes. State explicitly when tests could not be run and why.

## Security & Configuration

Keep credentials and local configuration out of version control. When configuration is introduced, provide sanitized examples and ignore generated output and local environment files.
