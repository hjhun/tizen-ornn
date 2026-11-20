#!/bin/bash
input=$(cat)
MODEL=$(printf '%s' "$input" | jq -r '.model.display_name')
CWD=$(printf '%s' "$input" | jq -r '.workspace.current_dir')
COST=$(printf '%s' "$input" | jq -r '.cost.total_cost_usd // 0')
PCT=$(printf '%s' "$input" | jq -r '.context_window.used_percentage // 0' | cut -d. -f1)
DURATION_MS=$(printf '%s' "$input" | jq -r '.cost.total_duration_ms // 0')
FIVE_H=$(printf '%s' "$input" | jq -r '.rate_limits.five_hour.used_percentage // empty')
WEEK=$(printf '%s' "$input" | jq -r '.rate_limits.seven_day.used_percentage // empty')

BOLD='\033[01m'; CYAN='\033[36m'; GREEN='\033[32m'; BLUE='\033[01;34m'
YELLOW='\033[33m'; RED='\033[31m'; RESET='\033[0m'

# PS1-style: user@host:dir

# Git branch with color (green=clean, cyan=ahead, red=dirty)
BRANCH=""
if git -C "$CWD" rev-parse --git-dir > /dev/null 2>&1; then
    BRANCH_NAME=$(git -C "$CWD" branch --show-current 2>/dev/null)
    if [ -n "$BRANCH_NAME" ]; then
        if git -C "$CWD" diff --quiet 2>/dev/null; then
            if git -C "$CWD" status 2>/dev/null | grep -q 'Your branch is ahead of'; then
                BRANCH_COLOR='\033[36m'
            else
                BRANCH_COLOR='\033[32m'
            fi
        else
            BRANCH_COLOR='\033[31m'
        fi
        BRANCH=" (${BRANCH_COLOR}${BRANCH_NAME}${RESET})"
    fi
fi

# Context window bar
if [ "$PCT" -ge 90 ]; then BAR_COLOR="$RED"
elif [ "$PCT" -ge 70 ]; then BAR_COLOR="$YELLOW"
else BAR_COLOR="$GREEN"; fi

FILLED=$((PCT / 10)); EMPTY=$((10 - FILLED))
printf -v FILL "%${FILLED}s"; printf -v PAD "%${EMPTY}s"
BAR="${FILL// /█}${PAD// /░}"

MINS=$((DURATION_MS / 60000)); SECS=$(((DURATION_MS % 60000) / 1000))
COST_FMT=$(printf '$%.4f' "$COST")

printf '%b%s@%s%b:%b%s%b' "${BOLD}${GREEN}" "$(whoami)" "$(hostname -s)" "$RESET" "${BOLD}${BLUE}" "$CWD" "$RESET"
if [ -n "$BRANCH" ]; then printf ' (%b%s%b)' "$BRANCH_COLOR" "$BRANCH_NAME" "$RESET"; fi
printf '\n'
printf '%b[%s]%b %b%s%b %s%% | %b%s%b | %sm %ss\n' "$CYAN" "$MODEL" "$RESET" "$BAR_COLOR" "$BAR" "$RESET" "$PCT" "$YELLOW" "$COST_FMT" "$RESET" "$MINS" "$SECS"

# Rate limits block
format_remaining() {
    local resets_at="$1"
    local now
    now=$(date +%s)
    local diff=$(( resets_at - now ))
    if [ "$diff" -le 0 ]; then
        echo "resetting"
        return
    fi
    local days=$(( diff / 86400 ))
    local hours=$(( (diff % 86400) / 3600 ))
    local mins=$(( (diff % 3600) / 60 ))
    if [ "$days" -gt 0 ]; then
        echo "${days}d ${hours}h ${mins}m"
    elif [ "$hours" -gt 0 ]; then
        echo "${hours}h ${mins}m"
    else
        echo "${mins}m"
    fi
}

make_bar() {
    local pct="$1"
    local filled=$(( ${pct%.*} / 10 ))
    [ "$filled" -gt 10 ] && filled=10
    local empty=$(( 10 - filled ))
    printf -v F "%${filled}s"; printf -v E "%${empty}s"
    echo "${F// /█}${E// /░}"
}

FIVE_H_AT=$(printf '%s' "$input" | jq -r '.rate_limits.five_hour.resets_at // empty')
WEEK_AT=$(printf '%s' "$input" | jq -r '.rate_limits.seven_day.resets_at // empty')

if [ -n "$FIVE_H" ] || [ -n "$WEEK" ]; then
    printf "${YELLOW}Rate limits:${RESET}\n"
    if [ -n "$FIVE_H" ]; then
        bar=$(make_bar "$FIVE_H")
        pct_fmt=$(printf '%.1f' "$FIVE_H")
        if [ -n "$FIVE_H_AT" ]; then
            remaining=$(format_remaining "$FIVE_H_AT")
            printf "  5-hour  ${YELLOW}[%s]${RESET} %s%%  resets in %s\n" "$bar" "$pct_fmt" "$remaining"
        else
            printf "  5-hour  ${YELLOW}[%s]${RESET} %s%%\n" "$bar" "$pct_fmt"
        fi
    fi
    if [ -n "$WEEK" ]; then
        bar=$(make_bar "$WEEK")
        pct_fmt=$(printf '%.1f' "$WEEK")
        if [ -n "$WEEK_AT" ]; then
            remaining=$(format_remaining "$WEEK_AT")
            printf "  7-day   ${YELLOW}[%s]${RESET} %s%%  resets in %s\n" "$bar" "$pct_fmt" "$remaining"
        else
            printf "  7-day   ${YELLOW}[%s]${RESET} %s%%\n" "$bar" "$pct_fmt"
        fi
    fi
fi
