# Autoline Zsh Integration Hook
# Binds Tab/completion and records command execution to autoline

__autoline_suggest() {
    if [[ -n "$BUFFER" && "$CURSOR" -eq "${#BUFFER}" ]]; then
        local ghost
        ghost=$(autoline suggest --line "$BUFFER" --cwd "$PWD" 2>/dev/null)
        if [[ -n "$ghost" ]]; then
            BUFFER="${BUFFER}${ghost}"
            CURSOR="${#BUFFER}"
            return 0
        fi
    fi
    zle expand-or-complete
}

zle -N __autoline_suggest
bindkey '^I' __autoline_suggest

autoload -Uz add-zsh-hook

__autoline_preexec() {
    autoline record --line "$1" --shell zsh --cwd "$PWD" >/dev/null 2>&1 &!
}

add-zsh-hook preexec __autoline_preexec
