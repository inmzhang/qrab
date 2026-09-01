# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_qrab_global_optspecs
    string join \n h/help V/version
end

function __fish_qrab_needs_command
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_qrab_global_optspecs) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function __fish_qrab_using_subcommand
    set -l cmd (__fish_qrab_needs_command)
    test -z "$cmd"
    and return 1
    contains -- $cmd[1] $argv
end

complete -c qrab -n "__fish_qrab_needs_command" -s h -l help -d 'Print help'
complete -c qrab -n "__fish_qrab_needs_command" -s V -l version -d 'Print version'
complete -c qrab -n "__fish_qrab_needs_command" -f -a "check" -d 'Check a circuit without rendering it'
complete -c qrab -n "__fish_qrab_needs_command" -f -a "compile" -d 'Compile a circuit to LaTeX, Typst, SVG, Quirk, or all of them'
complete -c qrab -n "__fish_qrab_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c qrab -n "__fish_qrab_using_subcommand check" -s h -l help -d 'Print help'
complete -c qrab -n "__fish_qrab_using_subcommand check" -s V -l version -d 'Print version'
complete -c qrab -n "__fish_qrab_using_subcommand compile" -s t -l target -d 'Output backend' -r -f -a "latex\t''
typst\t''
svg\t''
quirk\t''
all\t''"
complete -c qrab -n "__fish_qrab_using_subcommand compile" -s o -l output -d 'Output file; requires a single backend' -r -F
complete -c qrab -n "__fish_qrab_using_subcommand compile" -s h -l help -d 'Print help'
complete -c qrab -n "__fish_qrab_using_subcommand compile" -s V -l version -d 'Print version'
complete -c qrab -n "__fish_qrab_using_subcommand help; and not __fish_seen_subcommand_from check compile help" -f -a "check" -d 'Check a circuit without rendering it'
complete -c qrab -n "__fish_qrab_using_subcommand help; and not __fish_seen_subcommand_from check compile help" -f -a "compile" -d 'Compile a circuit to LaTeX, Typst, SVG, Quirk, or all of them'
complete -c qrab -n "__fish_qrab_using_subcommand help; and not __fish_seen_subcommand_from check compile help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
