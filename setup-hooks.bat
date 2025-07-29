@echo off
REM Script to set up git hooks for the project on Windows

echo Setting up git hooks...

REM Configure git to use the .githooks directory
git config core.hooksPath .githooks

echo.
echo Git hooks configured successfully!
echo The pre-commit hook will now run 'cargo fmt --check' before each commit.
echo.
echo To bypass the hook (not recommended), use: git commit --no-verify