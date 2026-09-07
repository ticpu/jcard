#!/bin/bash
# Bump Cargo.toml, commit, pin Cargo.lock on a detached commit, sign the tag.
#
# The detach dance is the error-prone part of a release: a chained command
# rejected mid-way (a hook, a denied permission) can commit Cargo.lock onto
# the branch it must never reach. Scripting it removes the chaining risk;
# each step here is checked before the next runs.
#
# Usage: scripts/release-tag.sh vX.Y.Z <changelog-file>

set -e

VERSION="$1"
CHANGELOG_FILE="$2"

if [ -z "$VERSION" ] || [ -z "$CHANGELOG_FILE" ]; then
	echo "Usage: $0 vX.Y.Z <changelog-file>" >&2
	exit 1
fi

case "$VERSION" in
v*) ;;
*)
	echo "VERSION must start with 'v' (got: $VERSION)" >&2
	exit 1
	;;
esac

if [ ! -f "$CHANGELOG_FILE" ]; then
	echo "Changelog file not found: $CHANGELOG_FILE" >&2
	exit 1
fi

BRANCH="$(git symbolic-ref --short HEAD)"
if [ "$BRANCH" != "master" ]; then
	echo "Must be on master (currently on $BRANCH)." >&2
	exit 1
fi

if [ -n "$(git status --porcelain)" ]; then
	echo "Working tree is dirty; commit or stash first." >&2
	git status --short >&2
	exit 1
fi

if git rev-parse -q --verify "refs/tags/$VERSION" >/dev/null; then
	echo "Tag $VERSION already exists." >&2
	exit 1
fi

CRATE_VERSION="${VERSION#v}"
sed -i "0,/^version = \".*\"/s//version = \"$CRATE_VERSION\"/" Cargo.toml
git add Cargo.toml
git commit -m "release: $VERSION"

git checkout --detach
if git symbolic-ref -q HEAD >/dev/null; then
	echo "Failed to detach HEAD; aborting before touching Cargo.lock." >&2
	exit 1
fi

cargo generate-lockfile
git add -f Cargo.lock
git commit -m "build: pin Cargo.lock for $VERSION"

git tag -as "$VERSION" -F "$CHANGELOG_FILE"

git switch "$BRANCH"

cat <<EOF
Tagged $VERSION on a detached commit off $BRANCH.
Review:  git show $VERSION
Push:    git push && wait for CI green on $BRANCH, then git push origin $VERSION
Publish: git checkout $VERSION && cargo publish --dry-run && cargo publish
EOF
