#!/bin/bash
set -e

echo "=========================================="
echo "   fastsync One-Key Publish Script        "
echo "=========================================="

# 0. Pre-flight check
echo "🔍 Running cargo check..."
cargo check

# --- 1. Version Management ---

# Extract Current Version
CURRENT_VERSION=$(grep "^version" Cargo.toml | head -n 1 | cut -d '"' -f 2)
IFS='.' read -r major minor patch <<< "$CURRENT_VERSION"

# Calculate Next Versions
NEXT_PATCH="$major.$minor.$((patch + 1))"
NEXT_MINOR="$major.$((minor + 1)).0"
NEXT_MAJOR="$((major + 1)).0.0"

echo "📌 Current Version: $CURRENT_VERSION"
echo ""
echo "Select release type:"
echo "  1) Patch (Bug fix)     -> $NEXT_PATCH [Default]"
echo "  2) Minor (New feature) -> $NEXT_MINOR"
echo "  3) Major (Breaking)    -> $NEXT_MAJOR"
echo "  4) Keep current        -> $CURRENT_VERSION"
echo ""

read -r -p "Choice [1]: " choice

NEW_VERSION=""
case "$choice" in
    2) NEW_VERSION="$NEXT_MINOR" ;;
    3) NEW_VERSION="$NEXT_MAJOR" ;;
    4) NEW_VERSION="$CURRENT_VERSION" ;;
    *) NEW_VERSION="$NEXT_PATCH" ;;
esac

# Update Cargo.toml if changed
if [ "$NEW_VERSION" != "$CURRENT_VERSION" ]; then
    echo "🆙 Updating version to $NEW_VERSION in Cargo.toml..."
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml
    else
        sed -i "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml
    fi
    VERSION=$NEW_VERSION
else
    echo "ℹ️  Keeping current version: $CURRENT_VERSION"
    VERSION=$CURRENT_VERSION
fi

TAG_NAME="v$VERSION"

# --- 2. Git Safety Check ---

if git rev-parse "$TAG_NAME" >/dev/null 2>&1; then
    echo "⚠️  Error: Tag $TAG_NAME already exists locally."
    echo "If you want to re-release this version, delete the tag first: git tag -d $TAG_NAME"
    exit 1
fi

# --- 3. Commit and Push ---

echo "📝 Committing changes..."
git add .

# Default commit message
read -r -p "Enter commit message (default: 'release: $VERSION'): " msg
if [ -z "$msg" ]; then
    msg="release: $VERSION"
fi

git commit -m "$msg"

echo "🏷️  Creating tag $TAG_NAME..."
git tag "$TAG_NAME"

echo "🚀 Pushing to origin (master and tag)..."
# Check if current branch is master
BRANCH=$(git rev-parse --abbrev-ref HEAD)
git push origin "$BRANCH"
git push origin "$TAG_NAME"

echo ""
echo "=========================================="
echo "🎉 Successfully published $TAG_NAME!"
echo "GitHub Actions will now build and release."
echo "=========================================="