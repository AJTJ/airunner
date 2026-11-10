#!/bin/sh
# scripts/trial/answer-prompts.sh <project> [seconds] (air-fkb): answer the development-channels
# prompt in every `<project>-*` tmux session with Enter, which picks "I am using this for local
# development", the recorded answer `.air/roles.md` lets the coordinator type after the owner's
# yes (air-yux); running `make trial` is that yes. One Enter per pane, only once the prompt is
# on it; a pane past its prompts shows the auto-mode status line and is left alone; nothing
# else is typed anywhere. Exit 1 naming the sessions that showed neither within <seconds>
# (180 by default). AIR_TMUX_SOCKET names a private tmux server, as the launchers take it.
set -eu
project=${1:?usage: answer-prompts.sh <project> [seconds]}
wait=${2:-180}

tmux_() {
    if [ -n "${AIR_TMUX_SOCKET:-}" ]; then tmux -L "$AIR_TMUX_SOCKET" "$@"; else tmux "$@"; fi
}

pending=$(tmux_ ls -F '#{session_name}' 2>/dev/null | grep "^$project-" || true)
[ -n "$pending" ] || { echo "answer-prompts: no $project-* tmux session is running"; exit 1; }
deadline=$(( $(date +%s) + wait ))
while [ -n "$pending" ] && [ "$(date +%s)" -lt "$deadline" ]; do
    still=
    for s in $pending; do
        pane=$(tmux_ capture-pane -p -t "=$s:" 2>/dev/null) || { echo "answer-prompts: $s is gone"; continue; }
        case $pane in
            *"development channels"*|*"local development"*)
                tmux_ send-keys -t "=$s:" Enter
                echo "answer-prompts: answered the development-channels prompt in $s" ;;
            *"auto mode on"*) ;;
            *) still="$still $s" ;;
        esac
    done
    pending=${still# }
    [ -z "$pending" ] || sleep 1
done
[ -z "$pending" ] || { echo "answer-prompts: no prompt seen within $wait s in:$(printf ' %s' $pending); read them with tmux capture-pane -p -t <session>"; exit 1; }
