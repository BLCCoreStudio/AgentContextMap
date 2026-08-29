#!/usr/bin/env bash
set -euo pipefail

readonly AGENTCONTEXT_VERSION="0.1.0-alpha.2"
readonly AGENTCONTEXT_TAG="v${AGENTCONTEXT_VERSION}"
readonly AGENTCONTEXT_ASSET="agentcontext-linux-x86_64"
readonly AGENTCONTEXT_SHA256="34f43883161ae8bdc8220e4ef0aa1226338e4f34ead0c45fadc5534c76578c26"
readonly AGENTCONTEXT_RELEASE_URL="https://github.com/BLCCoreStudio/AgentContextMap/releases/download/${AGENTCONTEXT_TAG}/${AGENTCONTEXT_ASSET}"

fail() {
  printf 'AgentContextMap Action: %s\n' "$1" >&2
  exit 1
}

if [[ "$(uname -s)" != "Linux" ]]; then
  fail "the current action supports Linux runners only."
fi

case "$(uname -m)" in
  x86_64|amd64)
    ;;
  *)
    fail "the current action supports x86_64 runners only."
    ;;
esac

for command_name in curl sha256sum realpath; do
  command -v "$command_name" >/dev/null 2>&1 \
    || fail "required command '${command_name}' is not available on this runner."
done

scan_path="${AGENTCONTEXT_INPUT_PATH:-.}"
target_path="${AGENTCONTEXT_INPUT_TARGET:-}"
report_format="${AGENTCONTEXT_INPUT_FORMAT:-terminal}"
fail_on_conflict="${AGENTCONTEXT_INPUT_FAIL_ON_CONFLICT:-false}"

case "$report_format" in
  terminal|json)
    ;;
  *)
    fail "format must be 'terminal' or 'json'."
    ;;
esac

case "$fail_on_conflict" in
  true|false)
    ;;
  *)
    fail "fail-on-conflict must be 'true' or 'false'."
    ;;
esac

[[ -d "$scan_path" ]] || fail "path must point to an existing directory."
scan_path="$(realpath -e -- "$scan_path")" || fail "path could not be resolved."

if [[ -n "${GITHUB_WORKSPACE:-}" ]]; then
  workspace="$(realpath -e -- "$GITHUB_WORKSPACE")" || fail "GITHUB_WORKSPACE could not be resolved."
  case "${scan_path}/" in
    "${workspace}/"*)
      ;;
    *)
      fail "path must stay inside GITHUB_WORKSPACE."
      ;;
  esac
fi

work_dir="$(mktemp -d -t agentcontext-action.XXXXXXXXXX)"
cleanup() {
  rm -rf -- "$work_dir"
}
trap cleanup EXIT

binary_path="${work_dir}/${AGENTCONTEXT_ASSET}"

printf 'AgentContextMap Action: downloading %s...\n' "$AGENTCONTEXT_TAG"
curl \
  --fail \
  --silent \
  --show-error \
  --location \
  --proto '=https' \
  --tlsv1.2 \
  --retry 3 \
  --retry-delay 1 \
  --retry-all-errors \
  --connect-timeout 10 \
  --max-time 120 \
  --output "$binary_path" \
  "$AGENTCONTEXT_RELEASE_URL"

printf '%s  %s\n' "$AGENTCONTEXT_SHA256" "$binary_path" | sha256sum --check --status \
  || fail "downloaded release binary failed SHA-256 verification."
chmod 0755 "$binary_path"

args=("$scan_path")

if [[ -n "$target_path" ]]; then
  args+=(--target "$target_path")
fi

if [[ "$report_format" == "json" ]]; then
  args+=(--json)
fi

if [[ "$fail_on_conflict" == "true" ]]; then
  args+=(--fail-on-conflict)
fi

printf 'AgentContextMap Action: inspecting %s\n' "$scan_path"
"$binary_path" "${args[@]}"
