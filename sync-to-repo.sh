#!/usr/bin/env bash
#
# sync-to-repo.sh — copy the OSlings project from this working directory into
# the publishable git repo, in a clean *unsolved* state.
#
# You solve exercises here in the working dir (so rv6/src/ fills up with your
# answers). The repo should ship the starting point instead, so this script:
#   - copies everything EXCEPT build artifacts, runtime state, .git, and your
#     mutable kernel scratch dir (rv6/src/), and
#   - resets the repo's rv6/src/ to just exercise 00's skeleton.
#
# The authored content you push lives under exercises/<NN>/{skeleton,solution}.
#
# Usage:
#   ./sync-to-repo.sh                 # dest defaults to ../REPO_OSlings
#   ./sync-to-repo.sh /path/to/repo   # or pass an explicit destination
#
# It does NOT touch git — run add/commit/push yourself in the repo afterward.

set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEST="${1:-$SRC/../REPO_OSlings}"

if [[ ! -d "$DEST" ]]; then
  echo "error: destination '$DEST' does not exist" >&2
  exit 1
fi
DEST="$(cd "$DEST" && pwd)"

echo "Syncing:  $SRC"
echo "      ->  $DEST"

# Mirror source into the repo. --delete keeps the repo tidy when files are
# renamed/removed, but excluded paths are always protected from deletion.
rsync -a --delete \
  --exclude='target/' \
  --exclude='.oslings/' \
  --exclude='.git/' \
  --exclude='rv6/src/' \
  "$SRC/" "$DEST/"

# Ship rv6/src in its clean, unsolved starting state: exercise 00's skeleton
# only. (Later exercises' files get staged into rv6/src by `oslings` at runtime
# and are never committed.)
SKELETON="$SRC/exercises/00_rust_kernel_basics/skeleton/main.rs"
mkdir -p "$DEST/rv6/src"
rm -f "$DEST"/rv6/src/*.rs
cp "$SKELETON" "$DEST/rv6/src/main.rs"

echo
echo "Done. rv6/src/ in the repo reset to the exercise-00 skeleton."
echo "Next:  cd \"$DEST\" && git add -A && git commit && git push"
