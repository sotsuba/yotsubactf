#!/usr/bin/env bash
# .githooks/log.sh — logging utilities for git hooks
# Source this file in each hook: source "$(dirname "$0")/log.sh"

# ── Colors & symbols ─────────────────────────────────────────────────────────

if [ -t 1 ] && [ "${NO_COLOR:-}" = "" ]; then
    RESET="\033[0m"
    BOLD="\033[1m"
    DIM="\033[2m"

    BLACK="\033[30m"
    RED="\033[31m"
    GREEN="\033[32m"
    YELLOW="\033[33m"
    BLUE="\033[34m"
    MAGENTA="\033[35m"
    CYAN="\033[36m"
    WHITE="\033[37m"

    BG_RED="\033[41m"
    BG_GREEN="\033[42m"
    BG_YELLOW="\033[43m"
    BG_BLUE="\033[44m"
else
    RESET="" BOLD="" DIM=""
    BLACK="" RED="" GREEN="" YELLOW="" BLUE="" MAGENTA="" CYAN="" WHITE=""
    BG_RED="" BG_GREEN="" BG_YELLOW="" BG_BLUE=""
fi

SYM_CHECK="✓"
SYM_CROSS="✗"
SYM_WARN="⚠"
SYM_RUN="›"
SYM_INFO="·"
SYM_ARROW="→"

# ── Timing ────────────────────────────────────────────────────────────────────

_HOOK_START_TIME=""

hook_timer_start() {
    _HOOK_START_TIME=$(date +%s%3N)    # milliseconds
}

hook_elapsed() {
    if [ -z "$_HOOK_START_TIME" ]; then echo "?ms"; return; fi
    local now
    now=$(date +%s%3N)
    local elapsed=$(( now - _HOOK_START_TIME ))
    if [ "$elapsed" -lt 1000 ]; then
        echo "${elapsed}ms"
    else
        local secs=$(( elapsed / 1000 ))
        local ms=$(( elapsed % 1000 ))
        echo "${secs}.${ms}s"
    fi
}

# ── Public API ────────────────────────────────────────────────────────────────

# log_header "pre-commit" "3 checks"
log_header() {
    local hook="${1:-hook}"
    local subtitle="${2:-}"
    local width=50
    local title="git hook · ${hook}"
    [ -n "$subtitle" ] && title="${title}  ${DIM}·${RESET}${BOLD}  ${subtitle}"

    printf "\n"
    printf "${DIM}┌%s┐${RESET}\n" "$(printf '%.0s─' $(seq 1 $width))"
    printf "${DIM}│${RESET}  ${BOLD}${CYAN}git hook${RESET} ${DIM}·${RESET} ${BOLD}${WHITE}%-*s${RESET}${DIM}│${RESET}\n" \
        $(( width - 3 )) "${hook}"
    [ -n "$subtitle" ] && \
    printf "${DIM}│${RESET}  ${DIM}%-*s│${RESET}\n" \
        $(( width - 1 )) "${subtitle}"
    printf "${DIM}└%s┘${RESET}\n" "$(printf '%.0s─' $(seq 1 $width))"
    printf "\n"

    hook_timer_start
}

# log_step "cargo fmt" "check formatting"
log_step() {
    local cmd="${1}"
    local desc="${2:-}"
    if [ -n "$desc" ]; then
        printf "  ${CYAN}${SYM_RUN}${RESET} ${BOLD}%-22s${RESET} ${DIM}${SYM_INFO} %s${RESET}\n" \
            "$cmd" "$desc"
    else
        printf "  ${CYAN}${SYM_RUN}${RESET} ${BOLD}%s${RESET}\n" "$cmd"
    fi
}

# log_ok "cargo fmt" "0.3s"
log_ok() {
    local label="${1}"
    local timing="${2:-}"
    if [ -n "$timing" ]; then
        printf "  ${GREEN}${SYM_CHECK}${RESET} ${WHITE}%-22s${RESET} ${DIM}(%s)${RESET}\n" \
            "$label" "$timing"
    else
        printf "  ${GREEN}${SYM_CHECK}${RESET} ${WHITE}%s${RESET}\n" "$label"
    fi
}

# log_skip "cargo test" "only runs on pre-push"
log_skip() {
    local label="${1}"
    local reason="${2:-skipped}"
    printf "  ${DIM}${SYM_INFO} %-22s (skipped — %s)${RESET}\n" \
        "$label" "$reason"
}

# log_warn "clippy" "2 warnings (non-fatal)"
log_warn() {
    local label="${1}"
    local msg="${2:-}"
    printf "  ${YELLOW}${SYM_WARN}${RESET} ${YELLOW}${BOLD}%-22s${RESET}  ${DIM}%s${RESET}\n" \
        "$label" "$msg"
}

# log_fail "cargo fmt" "failed to format 3 files"
log_fail() {
    local label="${1}"
    local reason="${2:-failed}"
    printf "  ${RED}${SYM_CROSS}${RESET} ${RED}${BOLD}%-22s${RESET}  %s\n" \
        "$label" "$reason"
}

# log_info "running: cargo fmt --all -- --check"
log_info() {
    printf "    ${DIM}${SYM_INFO} %s${RESET}\n" "$*"
}

# log_section "LINT"
log_section() {
    local label="${1}"
    printf "\n  ${DIM}── ${BOLD}${MAGENTA}%s${RESET} ${DIM}%s${RESET}\n" \
        "$label" "$(printf '%.0s─' $(seq 1 $(( 40 - ${#label} ))))"
}

# log_success "pre-commit" 3 0
log_success() {
    local hook="${1}"
    local total="${2:-0}"
    local skipped="${3:-0}"
    local passed=$(( total - skipped ))
    local elapsed
    elapsed=$(hook_elapsed)

    printf "\n  ${DIM}%s${RESET}\n" "$(printf '%.0s─' $(seq 1 46))"
    printf "  ${GREEN}${BOLD}${SYM_CHECK}  %s completed${RESET}" "$hook"
    [ "$total" -gt 0 ] && \
    printf "  ${DIM}·  %d/%d passed${RESET}" "$passed" "$total"
    printf "  ${DIM}·  %s${RESET}\n" "$elapsed"
    printf "  ${DIM}%s${RESET}\n\n" "$(printf '%.0s─' $(seq 1 46))"
}

# log_failure "pre-commit" "cargo clippy" "fix warnings before committing"
log_failure() {
    local hook="${1}"
    local failed_step="${2:-unknown}"
    local hint="${3:-}"

    printf "\n  ${DIM}%s${RESET}\n" "$(printf '%.0s─' $(seq 1 46))"
    printf "  ${RED}${BOLD}${SYM_CROSS}  %s failed${RESET}  ${DIM}·  %s${RESET}\n" \
        "$hook" "$failed_step"
    [ -n "$hint" ] && \
    printf "     ${DIM}${SYM_ARROW} %s${RESET}\n" "$hint"
    printf "  ${DIM}%s${RESET}\n\n" "$(printf '%.0s─' $(seq 1 46))"
}

# ── Helpers to wrap commands ──────────────────────────────────────────────────

# run_step "cargo fmt" "check formatting" cargo fmt --all -- --check
run_step() {
    local label="$1"
    local desc="$2"
    shift 2
    local cmd=("$@")

    local step_start
    step_start=$(date +%s%3N)

    log_step "$label" "$desc"
    log_info "${cmd[*]}"

    if "${cmd[@]}" 2>&1 | sed 's/^/    /'; then
        local step_end
        step_end=$(date +%s%3N)
        local step_elapsed=$(( step_end - step_start ))
        local timing
        if [ "$step_elapsed" -lt 1000 ]; then
            timing="${step_elapsed}ms"
        else
            timing="$(( step_elapsed / 1000 )).$(( step_elapsed % 1000 ))s"
        fi
        log_ok "$label" "$timing"
        return 0
    else
        log_fail "$label" "exit code $?"
        return 1
    fi
}
