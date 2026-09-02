
use builtin;
use str;

set edit:completion:arg-completer[qrab] = {|@words|
    fn spaces {|n|
        builtin:repeat $n ' ' | str:join ''
    }
    fn cand {|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }
    var command = 'qrab'
    for word $words[1..-1] {
        if (str:has-prefix $word '-') {
            break
        }
        set command = $command';'$word
    }
    var completions = [
        &'qrab'= {
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
            cand check 'Check a circuit without rendering it'
            cand compile 'Compile a circuit to LaTeX, Typst, SVG, Quirk, or all of them'
            cand install-skill 'Install the qrab agent skill in the current project'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'qrab;check'= {
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'qrab;compile'= {
            cand -t 'Output backend'
            cand --target 'Output backend'
            cand -o 'Output file; requires a single backend'
            cand --output 'Output file; requires a single backend'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'qrab;install-skill'= {
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'qrab;help'= {
            cand check 'Check a circuit without rendering it'
            cand compile 'Compile a circuit to LaTeX, Typst, SVG, Quirk, or all of them'
            cand install-skill 'Install the qrab agent skill in the current project'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'qrab;help;check'= {
        }
        &'qrab;help;compile'= {
        }
        &'qrab;help;install-skill'= {
        }
        &'qrab;help;help'= {
        }
    ]
    $completions[$command]
}
