#!/usr/bin/env bash
set -euo pipefail

readonly AGENTCONTEXT_ASSET="agentcontext-linux-x86_64"

action_ref="${AGENTCONTEXT_ACTION_REF:-}"
use_local_binary=false
local_binary=""

if [[ -z "$action_ref" ]]; then
  [[ -n "${GITHUB_ACTION_PATH:-}" ]] \
    || { printf 'AgentContextMap Action: GITHUB_ACTION_PATH is not available for a local Action invocation.\n' >&2; exit 1; }
  local_binary="${GITHUB_ACTION_PATH}/target/release/agentcontext"
  [[ -x "$local_binary" ]] \
    || { printf 'AgentContextMap Action: local Action invocation requires a current release build at %s.\n' "$local_binary" >&2; exit 1; }
  use_local_binary=true
elif [[ "$action_ref" =~ ^v[0-9]+(\.[0-9]+){0,2}(-[0-9A-Za-z.-]+)?$ ]]; then
  AGENTCONTEXT_TAG="$action_ref"
else
  [[ -n "${GITHUB_ACTION_PATH:-}" ]] \
    || { printf 'AgentContextMap Action: cannot resolve a non-tag Action ref without GITHUB_ACTION_PATH.\n' >&2; exit 1; }
  action_version="$(sed -n 's/^version = "\([0-9][0-9.]*\)"/\1/p' "${GITHUB_ACTION_PATH}/Cargo.toml" | head -n1)"
  [[ "$action_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] \
    || { printf 'AgentContextMap Action: could not resolve the release version from Cargo.toml.\n' >&2; exit 1; }
  AGENTCONTEXT_TAG="v${action_version}"
  printf 'AgentContextMap Action: resolved ref %s to release %s.\n' "$action_ref" "$AGENTCONTEXT_TAG"
fi

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

for command_name in realpath; do
  command -v "$command_name" >/dev/null 2>&1 \
    || fail "required command '${command_name}' is not available on this runner."
done

if [[ "$use_local_binary" != "true" ]]; then
  for command_name in curl sha256sum; do
    command -v "$command_name" >/dev/null 2>&1 \
      || fail "required command '${command_name}' is not available on this runner."
  done
fi

scan_path="${AGENTCONTEXT_INPUT_PATH:-.}"
target_path="${AGENTCONTEXT_INPUT_TARGET:-}"
report_format="${AGENTCONTEXT_INPUT_FORMAT:-terminal}"
sarif_input="${AGENTCONTEXT_INPUT_SARIF:-}"
fail_on_conflict="${AGENTCONTEXT_INPUT_FAIL_ON_CONFLICT:-false}"
job_summary="${AGENTCONTEXT_INPUT_JOB_SUMMARY:-true}"

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

case "$job_summary" in
  true|false)
    ;;
  *)
    fail "job-summary must be 'true' or 'false'."
    ;;
esac

[[ -d "$scan_path" ]] || fail "path must point to an existing directory."
scan_path="$(realpath -e -- "$scan_path")" || fail "path could not be resolved."

workspace=""
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

sarif_path=""
if [[ -n "$sarif_input" ]]; then
  [[ -n "$workspace" ]] || fail "sarif output requires GITHUB_WORKSPACE."
  [[ "$sarif_input" != /* ]] || fail "sarif must be a workspace-relative path."
  [[ "$sarif_input" != *$'\n'* && "$sarif_input" != *$'\r'* ]] \
    || fail "sarif path must not contain newlines."

  sarif_path="$(realpath -m -- "${workspace}/${sarif_input}")" || fail "sarif path could not be resolved."
  case "$sarif_path" in
    "${workspace}/"*)
      ;;
    *)
      fail "sarif output must stay inside GITHUB_WORKSPACE."
      ;;
  esac
  mkdir -p -- "$(dirname -- "$sarif_path")"
fi

if [[ "$use_local_binary" == "true" ]]; then
  binary_path="$local_binary"
  printf 'AgentContextMap Action: using current local release build.\n'
else
  readonly AGENTCONTEXT_TAG
  readonly AGENTCONTEXT_RELEASE_URL="https://github.com/BLCCoreStudio/AgentContextMap/releases/download/${AGENTCONTEXT_TAG}/${AGENTCONTEXT_ASSET}"
  readonly AGENTCONTEXT_CHECKSUM_URL="${AGENTCONTEXT_RELEASE_URL}.sha256"

  work_dir="$(mktemp -d -t agentcontext-action.XXXXXXXXXX)"
  cleanup() {
    rm -rf -- "$work_dir"
  }
  trap cleanup EXIT

  binary_path="${work_dir}/${AGENTCONTEXT_ASSET}"
  checksum_path="${binary_path}.sha256"

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
    --max-time 30 \
    --output "$checksum_path" \
    "$AGENTCONTEXT_CHECKSUM_URL"

  (
    cd "$work_dir"
    sha256sum --check --status "${AGENTCONTEXT_ASSET}.sha256"
  ) || fail "downloaded release binary failed SHA-256 verification."
  chmod 0755 "$binary_path"
fi

args=("$scan_path")

if [[ -n "$target_path" ]]; then
  args+=(--target "$target_path")
fi

if [[ "$report_format" == "json" ]]; then
  args+=(--json)
fi

if [[ -n "$sarif_path" ]]; then
  args+=(--sarif "$sarif_path")
  if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    printf 'sarif=%s\n' "$sarif_path" >> "$GITHUB_OUTPUT"
  fi
fi

if [[ "$fail_on_conflict" == "true" ]]; then
  args+=(--fail-on-conflict)
fi

printf 'AgentContextMap Action: inspecting %s\n' "$scan_path"

set +e
scan_output="$("$binary_path" "${args[@]}" 2>&1)"
scan_status=$?
set -e

printf '%s\n' "$scan_output"

if [[ "$job_summary" == "true" && -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  case "$scan_status" in
    0) summary_result="pass" ;;
    2) summary_result="conflict gate triggered" ;;
    *) summary_result="failed (exit ${scan_status})" ;;
  esac

  if ! {
    printf '## AgentContextMap\n\n'
    printf '**Result:** `%s`\n\n' "$summary_result"
    if [[ -n "$target_path" ]]; then
      printf '**Target:** `%s`\n\n' "$target_path"
    fi
    printf '### Report\n\n'
    printf '%s\n' "$scan_output" | sed 's/^/    /'
    printf '\n'
  } >> "$GITHUB_STEP_SUMMARY"; then
    printf 'AgentContextMap Action: warning: failed to append the job summary.\n' >&2
  fi
fi

exit "$scan_status"
