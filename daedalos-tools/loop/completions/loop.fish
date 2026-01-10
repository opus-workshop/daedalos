# Fish completion for loop command
# Copy to ~/.config/fish/completions/loop.fish

# Disable file completions by default
complete -c loop -f

# Main commands
complete -c loop -n "__fish_use_subcommand" -a start -d "Start a new loop"
complete -c loop -n "__fish_use_subcommand" -a status -d "Show loop status"
complete -c loop -n "__fish_use_subcommand" -a watch -d "Watch loop execution live"
complete -c loop -n "__fish_use_subcommand" -a pause -d "Pause a running loop"
complete -c loop -n "__fish_use_subcommand" -a resume -d "Resume a paused loop"
complete -c loop -n "__fish_use_subcommand" -a cancel -d "Cancel a loop"
complete -c loop -n "__fish_use_subcommand" -a inject -d "Inject context into loop"
complete -c loop -n "__fish_use_subcommand" -a checkpoint -d "Create named checkpoint"
complete -c loop -n "__fish_use_subcommand" -a rollback -d "Rollback to checkpoint"
complete -c loop -n "__fish_use_subcommand" -a history -d "Show iteration history"
complete -c loop -n "__fish_use_subcommand" -a list -d "List all loops"
complete -c loop -n "__fish_use_subcommand" -a template -d "Manage loop templates"
complete -c loop -n "__fish_use_subcommand" -a workflow -d "Run multi-loop workflow"
complete -c loop -n "__fish_use_subcommand" -a version -d "Show version"
complete -c loop -n "__fish_use_subcommand" -a help -d "Show help"

# Helper function to get loop IDs
function __loop_ids
    set -l state_dir "$HOME/.local/share/daedalos/loop/states"
    if test -d "$state_dir"
        for f in $state_dir/*.json
            basename $f .json
        end
    end
end

# start subcommand
complete -c loop -n "__fish_seen_subcommand_from start" -l promise -s p -d "Success condition command" -r
complete -c loop -n "__fish_seen_subcommand_from start" -l max-iterations -s n -d "Maximum iterations" -r
complete -c loop -n "__fish_seen_subcommand_from start" -l agent -d "Agent to use" -ra "opencode claude aider cursor custom auto"
complete -c loop -n "__fish_seen_subcommand_from start" -l agent-cmd -d "Custom agent command" -r
complete -c loop -n "__fish_seen_subcommand_from start" -l checkpoint -d "Checkpoint strategy" -ra "btrfs git none auto"
complete -c loop -n "__fish_seen_subcommand_from start" -l timeout -d "Per-iteration timeout" -r
complete -c loop -n "__fish_seen_subcommand_from start" -l best-of -d "Run N parallel branches" -r
complete -c loop -n "__fish_seen_subcommand_from start" -l template -d "Use template" -ra "tdd bugfix refactor feature security performance"
complete -c loop -n "__fish_seen_subcommand_from start" -l inject -d "Inject context from file" -rF
complete -c loop -n "__fish_seen_subcommand_from start" -l background -s b -d "Run in background"
complete -c loop -n "__fish_seen_subcommand_from start" -l notify -d "Send notification on completion"
complete -c loop -n "__fish_seen_subcommand_from start" -l help -s h -d "Show help"

# status subcommand
complete -c loop -n "__fish_seen_subcommand_from status" -a "(__loop_ids)" -d "Loop ID"
complete -c loop -n "__fish_seen_subcommand_from status" -l watch -s w -d "Continuously update"
complete -c loop -n "__fish_seen_subcommand_from status" -l json -d "Output as JSON"

# watch, pause, resume subcommands
complete -c loop -n "__fish_seen_subcommand_from watch pause resume" -a "(__loop_ids)" -d "Loop ID"

# cancel subcommand
complete -c loop -n "__fish_seen_subcommand_from cancel" -a "(__loop_ids)" -d "Loop ID"
complete -c loop -n "__fish_seen_subcommand_from cancel" -l rollback -d "Rollback to initial state"
complete -c loop -n "__fish_seen_subcommand_from cancel" -l keep -d "Keep current state"

# inject subcommand
complete -c loop -n "__fish_seen_subcommand_from inject" -a "(__loop_ids)" -d "Loop ID"
complete -c loop -n "__fish_seen_subcommand_from inject" -l file -d "Read context from file" -rF

# checkpoint subcommand
complete -c loop -n "__fish_seen_subcommand_from checkpoint" -a "(__loop_ids)" -d "Loop ID"

# rollback subcommand
complete -c loop -n "__fish_seen_subcommand_from rollback" -a "(__loop_ids)" -d "Loop ID"

# history subcommand
complete -c loop -n "__fish_seen_subcommand_from history" -a "(__loop_ids)" -d "Loop ID"
complete -c loop -n "__fish_seen_subcommand_from history" -l verbose -s v -d "Include agent output"
complete -c loop -n "__fish_seen_subcommand_from history" -l diff -d "Show diff for iteration" -r

# template subcommand
complete -c loop -n "__fish_seen_subcommand_from template; and not __fish_seen_subcommand_from list show create" -a list -d "List templates"
complete -c loop -n "__fish_seen_subcommand_from template; and not __fish_seen_subcommand_from list show create" -a show -d "Show template"
complete -c loop -n "__fish_seen_subcommand_from template; and not __fish_seen_subcommand_from list show create" -a create -d "Create template"
complete -c loop -n "__fish_seen_subcommand_from template; and __fish_seen_subcommand_from show create" -a "tdd bugfix refactor feature security performance" -d "Template name"

# workflow subcommand
complete -c loop -n "__fish_seen_subcommand_from workflow; and not __fish_seen_subcommand_from run list" -a run -d "Run workflow"
complete -c loop -n "__fish_seen_subcommand_from workflow; and not __fish_seen_subcommand_from run list" -a list -d "List workflows"
complete -c loop -n "__fish_seen_subcommand_from workflow; and __fish_seen_subcommand_from run" -l set -d "Set variable" -r

# list subcommand
complete -c loop -n "__fish_seen_subcommand_from list" -l status -d "Filter by status" -ra "pending running paused completed failed cancelled"
complete -c loop -n "__fish_seen_subcommand_from list" -l agent -d "Filter by agent" -ra "opencode claude aider cursor"
complete -c loop -n "__fish_seen_subcommand_from list" -l since -d "Filter by start time" -r
complete -c loop -n "__fish_seen_subcommand_from list" -l limit -d "Limit results" -r
complete -c loop -n "__fish_seen_subcommand_from list" -l json -d "Output as JSON"
