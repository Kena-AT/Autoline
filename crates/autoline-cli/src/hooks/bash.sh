# Autoline Bash Integration Hook
# Binds Tab/completion and records command execution to autoline

__autoline_suggest() {
    local line="${READLINE_LINE}"
    local point="${READLINE_POINT}"
    if [ -n "$line" ] && [ "$point" -eq "${#line}" ]; then
        local ghost
        ghost=$(autoline suggest --line "$line" --cwd "$PWD" 2>/dev/null)
        if [ -n "$ghost" ]; then
            READLINE_LINE="${line}${ghost}"
            READLINE_POINT="${#READLINE_LINE}"
        fi
    fi
}

__autoline_precmd() {
    local last_cmd
    last_cmd=$(history 1 | sed 's/^[ ]*[0-9]*[ ]*//')
    if [ -n "$last_cmd" ] && [ "$last_cmd" != "$__AUTOLINE_LAST_CMD" ]; then
        __AUTOLINE_LAST_CMD="$last_cmd"
        autoline record --line "$last_cmd" --shell bash --cwd "$PWD" >/dev/null 2>&1 &
    fi
}

PROMPT_COMMAND="__autoline_precmd; ${PROMPT_COMMAND}"
bind -x '"\t": __autoline_suggest'
