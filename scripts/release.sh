#!/usr/bin/env bash
#
# Releases dan: bumps the version in Cargo.toml and Cargo.lock, commits
# "Release X.Y.Z", tags it, and pushes the commit and the tag together. The
# tag starts the release workflow (.github/workflows/release.yml), which
# builds the binaries, publishes the GitHub release, and updates the
# Homebrew tap.
#
#   scripts/release.sh            patch: 0.4.2 -> 0.4.3
#   scripts/release.sh minor      0.4.2 -> 0.5.0
#   scripts/release.sh major      0.4.2 -> 1.0.0
#   scripts/release.sh 0.6.0      that version exactly
#
#   --notes FILE   after the workflow has published the release, replace its
#                  generated notes with FILE (e.g. docs/releases/0.5.0-foo.md).
#                  Needs the gh CLI, logged in.
#
# Nothing is changed unless master is checked out, clean, and level with
# origin/master, and the checks CI runs pass. It asks before it commits.

set -euo pipefail

die() {
	echo "release: $*" >&2
	exit 1
}
usage="usage: scripts/release.sh [major|minor|patch|X.Y.Z] [--notes FILE]"

bump=
notes=
while [ $# -gt 0 ]; do
	case "$1" in
	-n | --notes)
		[ $# -ge 2 ] || die "$usage"
		notes="$2"
		shift 2
		;;
	--notes=*)
		notes="${1#--notes=}"
		shift
		;;
	-h | --help)
		echo "$usage"
		exit 0
		;;
	-*) die "$usage" ;;
	*)
		[ -z "$bump" ] || die "$usage"
		bump="$1"
		shift
		;;
	esac
done
bump="${bump:-patch}"

cd "$(git rev-parse --show-toplevel)"

if [ -n "$notes" ]; then
	[ -s "$notes" ] || die "no notes file at $notes"
	notes="$(cd "$(dirname "$notes")" && pwd)/$(basename "$notes")"
	command -v gh >/dev/null || die "--notes needs the gh CLI"
	gh auth status >/dev/null 2>&1 || die "--notes needs gh to be logged in (gh auth login)"
fi

branch="$(git symbolic-ref --quiet --short HEAD)" || die "HEAD is detached"
[ "$branch" = master ] || die "on $branch, not master"
[ -z "$(git status --porcelain)" ] || die "the working tree has changes"
git fetch --quiet origin master
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/master)" ] ||
	die "master is not level with origin/master"

current="$(sed -n 's/^version = "\([0-9]*\.[0-9]*\.[0-9]*\)".*/\1/p' Cargo.toml | head -n 1)"
[ -n "$current" ] || die "no version in Cargo.toml"
IFS=. read -r major minor patch <<<"$current"
case "$bump" in
major) new="$((major + 1)).0.0" ;;
minor) new="$major.$((minor + 1)).0" ;;
patch) new="$major.$minor.$((patch + 1))" ;;
*)
	[[ "$bump" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "$usage"
	new="$bump"
	;;
esac
if [ "$new" = "$current" ] ||
	[ "$(printf '%s\n' "$current" "$new" | sort -V | tail -n 1)" != "$new" ]; then
	die "$new is not after $current"
fi
if git rev-parse --quiet --verify "refs/tags/$new" >/dev/null ||
	git ls-remote --exit-code --tags origin "refs/tags/$new" >/dev/null; then
	die "tag $new already exists"
fi

echo "Running the checks CI runs…"
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked

echo
echo "Release $current -> $new"
echo "  commit 'Release $new', tag $new, and push both to origin"
if [ -n "$notes" ]; then
	echo "  then wait for the release workflow and set the notes from $notes"
fi
read -r -p "Go ahead? [y/N] " answer
case "$answer" in
y | Y | yes) ;;
*) die "stopped; nothing was changed" ;;
esac

# Only the first `version =` line: the [package] one.
perl -pi -e "\$d ||= s/^version = \"[0-9.]+\"/version = \"$new\"/" Cargo.toml
# Cargo.lock too, so release builds can use --locked.
perl -0pi -e "s/(\[\[package\]\]\nname = \"dan\"\nversion = \")[0-9.]+/\${1}$new/" Cargo.lock
grep -q "^version = \"$new\"" Cargo.toml || die "Cargo.toml was not updated"
cargo metadata --locked --format-version 1 >/dev/null ||
	die "Cargo.lock is out of step with Cargo.toml"

git add Cargo.toml Cargo.lock
git commit --quiet -m "Release $new"
git tag "$new"
# Atomic, so the tag is never on the remote without the commit, or the
# commit without the tag.
if ! git push --atomic origin master "refs/tags/$new"; then
	echo >&2
	echo "release: the push failed. The commit and tag are still here; push" >&2
	echo "them with:  git push --atomic origin master refs/tags/$new" >&2
	echo "or undo them with:  git tag -d $new && git reset --hard HEAD~1" >&2
	exit 1
fi

echo
echo "Released $new. The release workflow is now building it."
url="$(git remote get-url origin)"
case "$url" in
*github.com[:/]*)
	repo="${url#*github.com[:/]}"
	echo "  https://github.com/${repo%.git}/actions"
	;;
esac

[ -n "$notes" ] || exit 0

later="  gh release edit $new --notes-file $notes"
echo
echo "Waiting for the release workflow to start…"
run=
for _ in $(seq 30); do
	run="$(gh run list --workflow release.yml --branch "$new" --limit 1 \
		--json databaseId --jq '.[0].databaseId // empty' 2>/dev/null || true)"
	[ -n "$run" ] && break
	sleep 2
done
if [ -z "$run" ]; then
	echo "release: the workflow run didn't show up. Once the release exists, set the notes with:" >&2
	echo "$later" >&2
	exit 1
fi
if ! gh run watch "$run" --exit-status --interval 15; then
	echo "release: the release workflow failed. Once it has published $new, set the notes with:" >&2
	echo "$later" >&2
	exit 1
fi
gh release edit "$new" --notes-file "$notes" >/dev/null ||
	die "couldn't set the notes; try:$(printf '\n')$later"
echo "Release notes for $new set from $notes"
