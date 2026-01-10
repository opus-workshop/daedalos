# Bash completion for loop command
# Source this file or add to ~/.bashrc:
#   source /path/to/loop.bash

_loop_completions() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local prev="${COMP_WORDS[COMP_CWORD-1]}"
    local cmd="${COMP_WORDS[1]}"

    # Main commands
    local commands="start status watch pause resume cancel inject checkpoint rollback history list template workflow version help"

    # Complete main command
    if [[ $COMP_CWORD -eq 1 ]]; then
        COMPREPLY=($(compgen -W "$commands" -- "$cur"))
        return
    fi

    # Command-specific completions
    case "$cmd" in
        start)
            case "$prev" in
                --agent)
                    COMPREPLY=($(compgen -W "opencode claude aider cursor custom auto" -- "$cur"))
                    ;;
                --checkpoint)
                    COMPREPLY=($(compgen -W "btrfs git none auto" -- "$cur"))
                    ;;
                --template)
                    COMPREPLY=($(compgen -W "tdd bugfix refactor feature security performance" -- "$cur"))
                    ;;
                --promise|-p|--agent-cmd|--inject|--max-iterations|-n|--timeout|--best-of)
                    # These need values, don't complete
                    COMPREPLY=()
                    ;;
                *)
                    COMPREPLY=($(compgen -W "--promise -p --max-iterations -n --agent --agent-cmd --checkpoint --timeout --best-of --template --inject --background -b --notify --help" -- "$cur"))
                    ;;
            esac
            ;;
        status|watch|pause|resume|cancel|history)
            case "$prev" in
                --watch|-w|--json|--verbose|-v)
                    COMPREPLY=()
                    ;;
                *)
                    if [[ "$cur" == -* ]]; then
                        case "$cmd" in
                            status)
                                COMPREPLY=($(compgen -W "--watch -w --json" -- "$cur"))
                                ;;
                            history)
                                COMPREPLY=($(compgen -W "--verbose -v --diff" -- "$cur"))
                                ;;
                            cancel)
                                COMPREPLY=($(compgen -W "--rollback --keep" -- "$cur"))
                                ;;
                        esac
                    else
                        # Complete with loop IDs from state directory
                        local state_dir="${XDG_DATA_HOME:-$HOME/.local/share}/daedalos/loop/states"
                        if [[ -d "$state_dir" ]]; then
                            local ids=$(ls "$state_dir" 2>/dev/null | sed 's/\.json$//')
                            COMPREPLY=($(compgen -W "$ids" -- "$cur"))
                        fi
                    fi
                    ;;
            esac
            ;;
        inject)
            case "$prev" in
                --file)
                    COMPREPLY=($(compgen -f -- "$cur"))
                    ;;
                *)
                    if [[ "$cur" == -* ]]; then
                        COMPREPLY=($(compgen -W "--file" -- "$cur"))
                    fi
                    ;;
            esac
            ;;
        checkpoint|rollback)
            # Complete with loop IDs
            local state_dir="${XDG_DATA_HOME:-$HOME/.local/share}/daedalos/loop/states"
            if [[ -d "$state_dir" ]]; then
                local ids=$(ls "$state_dir" 2>/dev/null | sed 's/\.json$//')
                COMPREPLY=($(compgen -W "$ids" -- "$cur"))
            fi
            ;;
        template)
            if [[ $COMP_CWORD -eq 2 ]]; then
                COMPREPLY=($(compgen -W "list show create" -- "$cur"))
            else
                case "${COMP_WORDS[2]}" in
                    show|create)
                        COMPREPLY=($(compgen -W "tdd bugfix refactor feature security performance" -- "$cur"))
                        ;;
                esac
            fi
            ;;
        workflow)
            if [[ $COMP_CWORD -eq 2 ]]; then
                COMPREPLY=($(compgen -W "run list" -- "$cur"))
            else
                case "${COMP_WORDS[2]}" in
                    run)
                        if [[ "$prev" == "--set" ]]; then
                            COMPREPLY=()
                        elif [[ "$cur" == -* ]]; then
                            COMPREPLY=($(compgen -W "--set" -- "$cur"))
                        else
                            COMPREPLY=($(compgen -f -X '!*.yaml' -- "$cur"))
                        fi
                        ;;
                esac
            fi
            ;;
        list)
            COMPREPLY=($(compgen -W "--status --agent --since --limit --json" -- "$cur"))
            ;;
    esac
}

complete -F _loop_completions loop
