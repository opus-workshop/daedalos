# Bash completion for daedalos
# Source this file or add to ~/.bashrc

_daedalos() {
    local cur prev words cword
    _init_completion || return

    # All available tools and commands
    local tools="loop verify undo sandbox project codex context error-db agent mcp-hub lsp-pool scratch"
    local human_tools="env notify session secrets pair handoff review focus metrics template container remote backup"
    local supervision="observe gates journal"
    local meta_cmds="status doctor tools version help"

    local all_cmds="$tools $human_tools $supervision $meta_cmds"

    case $prev in
        daedalos)
            COMPREPLY=( $(compgen -W "$all_cmds" -- "$cur") )
            return
            ;;
        loop)
            COMPREPLY=( $(compgen -W "start status watch pause resume cancel inject checkpoint rollback history list template workflow help" -- "$cur") )
            return
            ;;
        verify)
            COMPREPLY=( $(compgen -W "--quick --path --help" -- "$cur") )
            return
            ;;
        undo)
            COMPREPLY=( $(compgen -W "checkpoint last timeline to restore help" -- "$cur") )
            return
            ;;
        sandbox)
            COMPREPLY=( $(compgen -W "create list enter diff promote discard info run help" -- "$cur") )
            return
            ;;
        project)
            COMPREPLY=( $(compgen -W "summary tree search deps conventions help" -- "$cur") )
            return
            ;;
        codex)
            COMPREPLY=( $(compgen -W "search index status help" -- "$cur") )
            return
            ;;
        context)
            COMPREPLY=( $(compgen -W "estimate breakdown help" -- "$cur") )
            return
            ;;
        error-db)
            COMPREPLY=( $(compgen -W "match add list search help" -- "$cur") )
            return
            ;;
        agent)
            COMPREPLY=( $(compgen -W "spawn list focus kill search send inbox broadcast signal lock claim workflow snapshot restore request-help help" -- "$cur") )
            return
            ;;
        mcp-hub)
            COMPREPLY=( $(compgen -W "status warm list restart logs call help" -- "$cur") )
            return
            ;;
        lsp-pool)
            COMPREPLY=( $(compgen -W "status warm cool list query languages logs restart help" -- "$cur") )
            return
            ;;
        scratch)
            COMPREPLY=( $(compgen -W "new list destroy enter help" -- "$cur") )
            return
            ;;
        observe)
            COMPREPLY=( $(compgen -W "--help --version" -- "$cur") )
            return
            ;;
        gates)
            COMPREPLY=( $(compgen -W "check level set config history help" -- "$cur") )
            return
            ;;
        journal)
            COMPREPLY=( $(compgen -W "what events summary log help" -- "$cur") )
            return
            ;;
        env)
            COMPREPLY=( $(compgen -W "enter leave list current help" -- "$cur") )
            return
            ;;
        notify)
            COMPREPLY=( $(compgen -W "send test config help" -- "$cur") )
            return
            ;;
        session)
            COMPREPLY=( $(compgen -W "save restore list delete help" -- "$cur") )
            return
            ;;
        secrets)
            COMPREPLY=( $(compgen -W "get set delete list inject init help" -- "$cur") )
            return
            ;;
        pair)
            COMPREPLY=( $(compgen -W "start join leave status help" -- "$cur") )
            return
            ;;
        handoff)
            COMPREPLY=( $(compgen -W "create accept list help" -- "$cur") )
            return
            ;;
        review)
            COMPREPLY=( $(compgen -W "request list accept complete help" -- "$cur") )
            return
            ;;
        focus)
            COMPREPLY=( $(compgen -W "start stop status config help" -- "$cur") )
            return
            ;;
        metrics)
            COMPREPLY=( $(compgen -W "show today week month export help" -- "$cur") )
            return
            ;;
        template)
            COMPREPLY=( $(compgen -W "list show create apply help" -- "$cur") )
            return
            ;;
        container)
            COMPREPLY=( $(compgen -W "run build list stop rm logs exec help" -- "$cur") )
            return
            ;;
        remote)
            COMPREPLY=( $(compgen -W "connect list add remove sync help" -- "$cur") )
            return
            ;;
        backup)
            COMPREPLY=( $(compgen -W "create restore list delete verify help" -- "$cur") )
            return
            ;;
        workflow)
            COMPREPLY=( $(compgen -W "list show start status stop help" -- "$cur") )
            return
            ;;
        signal)
            COMPREPLY=( $(compgen -W "complete wait check clear" -- "$cur") )
            return
            ;;
        lock)
            COMPREPLY=( $(compgen -W "acquire release check list" -- "$cur") )
            return
            ;;
        claim)
            COMPREPLY=( $(compgen -W "create release check list" -- "$cur") )
            return
            ;;
        spawn)
            COMPREPLY=( $(compgen -W "-n --name -t --template -p --project --no-focus --prompt" -- "$cur") )
            return
            ;;
        --template|-t)
            COMPREPLY=( $(compgen -W "explorer implementer reviewer debugger planner tester watcher" -- "$cur") )
            return
            ;;
        start)
            # Could be loop start or workflow start
            if [[ "${words[1]}" == "loop" ]]; then
                COMPREPLY=( $(compgen -W "--promise --max-iterations --agent --checkpoint --timeout --best-of --background --notify --orchestrate" -- "$cur") )
            elif [[ "${words[1]}" == "workflow" ]]; then
                COMPREPLY=( $(compgen -W "feature review bugfix tdd refactor" -- "$cur") )
            fi
            return
            ;;
        --agent)
            COMPREPLY=( $(compgen -W "opencode claude aider custom auto" -- "$cur") )
            return
            ;;
        --checkpoint)
            COMPREPLY=( $(compgen -W "btrfs git none auto" -- "$cur") )
            return
            ;;
        level)
            if [[ "${words[1]}" == "gates" ]]; then
                COMPREPLY=( $(compgen -W "autonomous supervised collaborative assisted manual" -- "$cur") )
            fi
            return
            ;;
    esac

    # Default to file completion
    _filedir
}

complete -F _daedalos daedalos
