# Autoline ZLE Widget
# This widget provides inline autocomplete suggestions for zsh.

autoload -Uz add-zsh-widget

# Initialize the daemon connection
_AUTOLINE_DAEMON_SOCK="${AUTOLINE_SOCKET_PATH:-$HOME/.local/share/autoline/autoline.sock}"
_AUTOLINE_BIN="${AUTOLINE_BIN:-autolined}"

# Send current line to daemon and get suggestion
autoline-suggest() {
    local line="${BUFFER}"
    local cwd="$(pwd)"

    # Don't suggest if line is empty or starts with a known binary pattern
    local first_word="${line%% *}"
    local first_word_trimmed="${line%%${first_word}*}"
    first_word_trimmed="${first_word_trimmed# }"

    # Skip if line is just whitespace
    if [[ -z "${line// /}" ]]; then
        return
    fi

    # Send suggestion request to daemon
    local response
    response=$(echo -n "${line}" "${cwd}" | timeout 3 \
        socat - UNIX-CONNECT:"${_AUTOLINE_DAEMON_SOCK}" 2>/dev/null || echo "")

    if [[ -z "${response}" ]]; then
        return
    fi

    # Parse response: format is "suggestion|source|confidence"
    local suggestion="${response%%|*}"
    local rest="${response#*|}"
    local source="${rest%%|*}"
    local confidence="${rest#*|}"

    # Display suggestion using POSTDISPLAY
    if [[ -n "${suggestion}" ]]; then
        # Remove leading/trailing whitespace
        suggestion="${suggestion#"${suggestion%%[![:space:]]*}"}"
        suggestion="${suggestion%"${suggestion##[![:space:]]}"}"

        # Only show if suggestion is not already in the buffer
        if [[ "${BUFFER}" != *"${suggestion}"* ]] || [[ ${#BUFFER} -lt 200 ]]; then
            # Use POSTDISPLAY to show greyed-out suggestion
            local cursor_pos=$(( CURSOR + 1 ))
            local suggestion_start=$(( #BUFFER - cursor_pos ))

            # Print the suggestion after the cursor using ANSI escape codes
            # Grey color code: \033[90m
            # Reset: \033[0m
            BUFFER="${BUFFER}\033[90m${suggestion}\033[0m"

            # Move cursor to after the suggestion
            POSTDISPLAY="\033[90m${suggestion}\033[0m"
            zle reset-prompt
        fi
    fi
}

# Accept the suggestion (bind to Tab and →)
autoline-accept-suggestion() {
    # If POSTDISPLAY is set, append it to the buffer and clear it
    if [[ -n "${POSTDISPLAY}" ]]; then
        BUFFER="${BUFFER}${POSTDISPLAY}"
        POSTDISPLAY=""
        zle reset-prompt
    else
        # Just accept the current word
        zle accepted-line
    fi
}

# Bind widgets
zle -N autoline-suggest
zle -N autoline-accept-suggestion

# Bind to self-insert (every keystroke) and clear-screen
bindkey '^M' autoline-accept-suggestion  # Enter key
bindkey '^[[Z' autoline-accept-suggestion  # Shift+Tab (some systems)
bindkey '^[[7~' autoline-suggest  # Home key (use as trigger)
bindkey '^[^[[Z' autoline-suggest  # Ctrl+Shift+Tab alternative

# Also bind on every keystroke for continuous suggestions
# Using hyde pattern: re-suggest after every command edit
autoload -Uz up-line-or-beginning-search down-line-or-beginning-search
zle -N up-line-or-beginning-search
zle -N down-line-or-beginning-search