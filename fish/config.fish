#dracula theme
fish_config theme choose "Dracula Official"
## Set values
# Hide welcome message & ensure we are reporting fish as shell
set fish_greeting
set VIRTUAL_ENV_DISABLE_PROMPT 1
set -x MANPAGER "sh -c 'col -bx | bat -l man -p'"
set -x SHELL /usr/bin/fish
## Export variable need for qt-theme
if type qtile >>/dev/null 2>&1
    set -x QT_QPA_PLATFORMTHEME qt5ct
end
# Set settings for https://github.com/franciscolourenco/done
set -U __done_min_cmd_duration 10000
set -U __done_notification_urgency_level low
## Environment setup
# Apply .profile: use this to put fish compatible .profile stuff in
if test -f ~/.fish_profile
    source ~/.fish_profile
end
# Add ~/.local/bin to PATH
if test -d ~/.local/bin
    if not contains -- ~/.local/bin $PATH
        set -p PATH ~/.local/bin
    end
end
# Add depot_tools to PATH
if test -d ~/Applications/depot_tools
    if not contains -- ~/Applications/depot_tools $PATH
        set -p PATH ~/Applications/depot_tools
    end
end
## Starship prompt
if status --is-interactive
    source ("/usr/local/bin/starship" init fish --print-full-init | psub)
end
## Advanced command-not-found hook
#source /usr/share/doc/find-the-command/ftc.fish
## Functions
# Functions needed for !! and !$ https://github.com/oh-my-fish/plugin-bang-bang
function __history_previous_command
    switch (commandline -t)
        case "!"
            commandline -t $history[1]
            commandline -f repaint
        case "*"
            commandline -i !
    end
end
function __history_previous_command_arguments
    switch (commandline -t)
        case "!"
            commandline -t ""
            commandline -f history-token-search-backward
        case "*"
            commandline -i '$'
    end
end

function pi-vnc
    set host jason@192.168.1.76
    set lport 5901
    set rmap "$lport:localhost:5900"

    # clean old tunnel
    set pid (lsof -t -iTCP:$lport -sTCP:LISTEN 2>/dev/null)
    test -n "$pid"; and kill $pid 2>/dev/null; and sleep 0.2

    # robust SSH tunnel with keepalives
    ssh -f -N -L $rmap -o ServerAliveInterval=15 -o ServerAliveCountMax=3 \
        -o ExitOnForwardFailure=yes -o StrictHostKeyChecking=accept-new $host

    # TigerVNC: fullscreen, lossless, proper cursor
    gamescope -b -W 2304 -H 1296 -w 2560 -h 1440 -S stretch -F fsr \
        --force-windows-fullscreen -- \
        vncviewer localhost:5901 \
        -FullScreen \
        -Shared=1 -NoJPEG=1 -CompressLevel=0 -QualityLevel=9 \
        -PreferredEncoding=ZRLE -CursorType=System -AlwaysCursor=1 -MenuKey=F9 &
end


#function pi-vnc
#    set host jason@192.168.1.76
#    set tunnel 5901:localhost:5900
#
#    # Clean stale listener
#    set -l pid (lsof -t -iTCP:5901 -sTCP:LISTEN 2>/dev/null)
#    if test -n "$pid"; kill $pid 2>/dev/null; sleep 0.2; end
#
#    # Make the tunnel; fail fast if auth/hostkey not OK
#    ssh -f -N -L $tunnel -o ExitOnForwardFailure=yes -o StrictHostKeyChecking=accept-new $host
#
#    # Launch 1440p VNC
#    #vinagre vnc://localhost:5901 --geometry=2560x1440 &
#    # or --fullscreen
#
#    vinagre vnc://localhost:5901 --fullscreen &
#   
#end


#if [ "$fish_key_bindings" = fish_vi_key_bindings ]

#    bind -Minsert ! __history_previous_command
#    bind -Minsert '$' __history_previous_command_arguments
#else
#    bind ! __history_previous_command
#    bind '$' __history_previous_command_arguments
#end
# Fish command history
function history
    builtin history --show-time='%F %T '
end
function backup --argument filename
    cp $filename $filename.bak
end
# Copy DIR1 DIR2
function copy
    set count (count $argv | tr -d \n)
    if test "$count" = 2; and test -d "$argv[1]"
        set from (echo $argv[1] | string trim --right --chars=/)
        set to (echo $argv[2])
        command cp -r $from $to
    else
        command cp $argv
    end
end
# Cleanup local orphaned packages
function cleanup
    while pacman -Qdtq
        sudo pacman -R (pacman -Qdtq)
    end
end
## Useful aliases
# Replace ls with eza
alias ls 'eza -al --color=always --group-directories-first --icons' # preferred listing
alias la 'eza -a --color=always --group-directories-first --icons' # all files and dirs
alias ll 'eza -l --color=always --group-directories-first --icons' # long format
alias lt 'eza -aT --color=always --group-directories-first --icons' # tree listing
alias l. 'eza -ald --color=always --group-directories-first --icons .*' # show only dotfiles
# Replace some more things with better alternatives
alias cat 'bat --style header --style snip --style changes --style header'
if not test -x /usr/bin/yay; and test -x /usr/bin/paru
    alias yay paru
end
# Common use
#not the way
#alias cd 'z'
alias .. 'cd ..'
alias ... 'cd ../..'
alias .... 'cd ../../..'
alias ..... 'cd ../../../..'
alias ...... 'cd ../../../../..'
alias big 'expac -H M "%m\t%n" | sort -h | nl' # Sort installed packages according to size in MB (expac must be installed)
alias dir 'dir --color=auto'
alias fixpacman 'sudo rm /var/lib/pacman/db.lck'
alias gitpkg 'pacman -Q | grep -i "\-git" | wc -l' # List amount of -git packages
alias grep 'ugrep --color=auto'
alias egrep 'ugrep -E --color=auto'
alias fgrep 'ugrep -F --color=auto'
alias grubup 'sudo update-grub'
alias hw 'hwinfo --short' # Hardware Info
alias ip 'ip -color'
alias psmem 'ps auxf | sort -nr -k 4'
alias psmem10 'ps auxf | sort -nr -k 4 | head -10'
alias rmpkg 'sudo pacman -Rdd'
alias tarnow 'tar -acf '
alias untar 'tar -zxvf '
alias upd /usr/bin/garuda-update
alias vdir 'vdir --color=auto'
alias wget 'wget -c '
# Get fastest mirrors
alias mirror 'sudo reflector -f 30 -l 30 --number 10 --verbose --save /etc/pacman.d/mirrorlist'
alias mirrora 'sudo reflector --latest 50 --number 20 --sort age --save /etc/pacman.d/mirrorlist'
alias mirrord 'sudo reflector --latest 50 --number 20 --sort delay --save /etc/pacman.d/mirrorlist'
alias mirrors 'sudo reflector --latest 50 --number 20 --sort score --save /etc/pacman.d/mirrorlist'
# Help people new to Arch
alias apt 'man pacman'
alias apt-get 'man pacman'
alias please sudo
alias tb 'nc termbin.com 9999'
alias helpme 'echo "To print basic information about a command use tldr <command>"'
alias pacdiff 'sudo -H DIFFPROG=meld pacdiff'
# Get the error messages from journalctl
alias jctl 'journalctl -p 3 -xb'
# Recent installed packages
alias rip 'expac --timefmt="%Y-%m-%d %T" "%l\t%n %v" | sort | tail -200 | nl'
## Run fastfetch if session is interactive
#if status --is-interactive && type -q fastfetch
#   fastfetch --load-config neofetch
#end
#replace with pfetch for more minimal
export PF_ASCII="debian"

#set default editor to nvim
set -Ux EDITOR nvim
set -Ux VISUAL nvim

pfetch
starship init fish | source
zoxide init --cmd cd fish | source
if test -d ~/.atuin/bin
    if not contains -- ~/.atuin/bin $PATH
        set -p PATH ~/.atuin/bin
    end
end
atuin init fish | source

# Starship/fish prompt repaint fixes
# Ensure consistent width handling for emojis/ambiguous glyphs
set -g fish_emoji_width 2
set -g fish_ambiguous_width 1
# Disable right prompt entirely to avoid stale characters
function fish_right_prompt; end
