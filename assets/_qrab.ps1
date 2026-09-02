
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'qrab' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'qrab'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'qrab' {
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('check', 'check', [CompletionResultType]::ParameterValue, 'Check a circuit without rendering it')
            [CompletionResult]::new('compile', 'compile', [CompletionResultType]::ParameterValue, 'Compile a circuit to LaTeX, Typst, SVG, Quirk, or all of them')
            [CompletionResult]::new('import-quirk', 'import-quirk', [CompletionResultType]::ParameterValue, 'Convert a Quirk URL to qrab source')
            [CompletionResult]::new('install-skill', 'install-skill', [CompletionResultType]::ParameterValue, 'Install the qrab agent skill in the current project')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'qrab;check' {
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'qrab;compile' {
            [CompletionResult]::new('-t', '-t', [CompletionResultType]::ParameterName, 'Output backend')
            [CompletionResult]::new('--target', '--target', [CompletionResultType]::ParameterName, 'Output backend')
            [CompletionResult]::new('-o', '-o', [CompletionResultType]::ParameterName, 'Output file; requires a single backend')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'Output file; requires a single backend')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'qrab;import-quirk' {
            [CompletionResult]::new('-o', '-o', [CompletionResultType]::ParameterName, 'Output `.qrab` file; writes to stdout when omitted')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'Output `.qrab` file; writes to stdout when omitted')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'qrab;install-skill' {
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'qrab;help' {
            [CompletionResult]::new('check', 'check', [CompletionResultType]::ParameterValue, 'Check a circuit without rendering it')
            [CompletionResult]::new('compile', 'compile', [CompletionResultType]::ParameterValue, 'Compile a circuit to LaTeX, Typst, SVG, Quirk, or all of them')
            [CompletionResult]::new('import-quirk', 'import-quirk', [CompletionResultType]::ParameterValue, 'Convert a Quirk URL to qrab source')
            [CompletionResult]::new('install-skill', 'install-skill', [CompletionResultType]::ParameterValue, 'Install the qrab agent skill in the current project')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'qrab;help;check' {
            break
        }
        'qrab;help;compile' {
            break
        }
        'qrab;help;import-quirk' {
            break
        }
        'qrab;help;install-skill' {
            break
        }
        'qrab;help;help' {
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
