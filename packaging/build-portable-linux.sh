#!/usr/bin/env bash
# Package a release payload containing sibling GUI, CLI, kernel and helper scripts.
set -euo pipefail
payload=$(realpath "$1")
output=$(realpath -m "$2")
for input in tizen-ornn tizen-ornn-gui scripts/linux-backend kernel/bzImage; do
    test -f "$payload/$input"
done
bundle=$(mktemp)
trap 'rm -f "$bundle"' EXIT
project=$(cd "$(dirname "$0")/.." && pwd)
tar -czf "$bundle" -C "$payload" --exclude=./LICENSE . -C "$project" LICENSE
digest=$(sha256sum "$bundle" | cut -d' ' -f1)
{
    echo '#!/usr/bin/env bash'
    printf "digest='%s'\n" "$digest"
    cat <<'LAUNCH'
set -euo pipefail
umask 077
file=$(realpath "$0")
export TIZEN_ORNN_PORTABLE_DIR="$(dirname "$file")/Tizen Ornn Portable"
mkdir -p "$TIZEN_ORNN_PORTABLE_DIR"
runtime="$TIZEN_ORNN_PORTABLE_DIR/runtime-$digest"
if [[ ! -d $runtime ]]; then
    stage=$(mktemp -d "$TIZEN_ORNN_PORTABLE_DIR/.extract.XXXXXXXX")
    trap 'rm -rf -- "$stage"' EXIT
    offset=$(awk '/^__ARCHIVE__$/ { print NR + 1; exit }' "$file")
    tail -n +"$offset" "$file" > "$stage/payload.gz"
    (cd "$stage" && echo "$digest  payload.gz" | sha256sum --check --status)
    mkdir "$stage/runtime"
    tar -xzf "$stage/payload.gz" -C "$stage/runtime"
    mv -T "$stage/runtime" "$runtime" || [[ -d $runtime ]]
    rm -rf -- "$stage"
    trap - EXIT
fi
exec "$runtime/tizen-ornn-gui"
__ARCHIVE__
LAUNCH
    cat "$bundle"
} > "$output"
chmod 755 "$output"
