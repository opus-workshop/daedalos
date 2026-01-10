#compdef loop

# Zsh completion for loop command
# Add to fpath or source directly

_loop() {
    local -a commands
    commands=(
        'start:Start a new loop'
        'status:Show loop status'
        'watch:Watch loop execution live'
        'pause:Pause a running loop'
        'resume:Resume a paused loop'
        'cancel:Cancel a loop'
        'inject:Inject context into loop'
        'checkpoint:Create named checkpoint'
        'rollback:Rollback to checkpoint'
        'history:Show iteration history'
        'list:List all loops'
        'template:Manage loop templates'
        'workflow:Run multi-loop workflow'
        'version:Show version'
        'help:Show help'
    )

    local -a agents
    agents=(opencode claude aider cursor custom auto)

    local -a checkpoints
    checkpoints=(btrfs git none auto)

    local -a templates
    templates=(tdd bugfix refactor feature security performance)

    _arguments -C \
        '1: :->command' \
        '*:: :->args'

    case $state in
        command)
            _describe -t commands 'loop command' commands
            ;;
        args)
            case $words[1] in
                start)
                    _arguments \
                        '1:prompt:' \
                        '--promise[Success condition command]:command:' \
                        '-p[Success condition command]:command:' \
                        '--max-iterations[Maximum iterations]:number:' \
                        '-n[Maximum iterations]:number:' \
                        '--agent[Agent to use]:agent:($agents)' \
                        '--agent-cmd[Custom agent command]:command:' \
                        '--checkpoint[Checkpoint strategy]:strategy:($checkpoints)' \
                        '--timeout[Per-iteration timeout]:seconds:' \
                        '--best-of[Run N parallel branches]:number:' \
                        '--template[Use template]:template:($templates)' \
                        '--inject[Inject context from file]:file:_files' \
                        '--background[Run in background]' \
                        '-b[Run in background]' \
                        '--notify[Send notification on completion]' \
                        '--help[Show help]'
                    ;;
                status)
                    _arguments \
                        '1:loop-id:_loop_ids' \
                        '--watch[Continuously update]' \
                        '-w[Continuously update]' \
                        '--json[Output as JSON]'
                    ;;
                watch|pause|resume)
                    _arguments '1:loop-id:_loop_ids'
                    ;;
                cancel)
                    _arguments \
                        '1:loop-id:_loop_ids' \
                        '--rollback[Rollback to initial state]' \
                        '--keep[Keep current state]'
                    ;;
                inject)
                    _arguments \
                        '1:loop-id:_loop_ids' \
                        '2:context:' \
                        '--file[Read context from file]:file:_files'
                    ;;
                checkpoint)
                    _arguments \
                        '1:loop-id:_loop_ids' \
                        '2:name:'
                    ;;
                rollback)
                    _arguments \
                        '1:loop-id:_loop_ids' \
                        '2:checkpoint:'
                    ;;
                history)
                    _arguments \
                        '1:loop-id:_loop_ids' \
                        '--verbose[Include agent output]' \
                        '-v[Include agent output]' \
                        '--diff[Show diff for iteration]:iteration:'
                    ;;
                template)
                    local -a subcmds
                    subcmds=(list show create)
                    _arguments \
                        '1:subcommand:($subcmds)' \
                        '2:template:($templates)'
                    ;;
                workflow)
                    local -a subcmds
                    subcmds=(run list)
                    _arguments \
                        '1:subcommand:($subcmds)' \
                        '2:file:_files -g "*.yaml"' \
                        '*--set[Set variable]:key=value:'
                    ;;
                list)
                    _arguments \
                        '--status[Filter by status]:status:(pending running paused completed failed cancelled)' \
                        '--agent[Filter by agent]:agent:($agents)' \
                        '--since[Filter by start time]:time:' \
                        '--limit[Limit results]:number:' \
                        '--json[Output as JSON]'
                    ;;
            esac
            ;;
    esac
}

# Helper function to complete loop IDs
_loop_ids() {
    local state_dir="${XDG_DATA_HOME:-$HOME/.local/share}/daedalos/loop/states"
    if [[ -d "$state_dir" ]]; then
        local -a ids
        ids=(${state_dir}/*.json(:t:r))
        _describe -t loop-ids 'loop ID' ids
    fi
}

_loop "$@"
