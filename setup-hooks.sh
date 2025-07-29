#!/bin/bash
# Script to set up git hooks for the project

echo "Setting up git hooks..."

# Configure git to use the .githooks directory
git config core.hooksPath .githooks

echo "Git hooks configured successfully!"
echo "The pre-commit hook will now run 'cargo fmt --check' before each commit."
echo ""
echo "To bypass the hook (not recommended), use: git commit --no-verify"