
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'boundbook' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'boundbook'
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
        'boundbook' {
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('docs', 'docs', [CompletionResultType]::ParameterValue, 'Print help')
            [CompletionResult]::new('create', 'create', [CompletionResultType]::ParameterValue, 'Create a BBF file from images')
            [CompletionResult]::new('info', 'info', [CompletionResultType]::ParameterValue, 'Display BBF file information')
            [CompletionResult]::new('verify', 'verify', [CompletionResultType]::ParameterValue, 'Verify BBF file integrity')
            [CompletionResult]::new('extract', 'extract', [CompletionResultType]::ParameterValue, 'Extract pages from a BBF file')
            [CompletionResult]::new('from-cbz', 'from-cbz', [CompletionResultType]::ParameterValue, 'Convert CBZ archive to BBF format')
            [CompletionResult]::new('read', 'read', [CompletionResultType]::ParameterValue, 'Read a BBF file in the terminal')
            [CompletionResult]::new('complete', 'complete', [CompletionResultType]::ParameterValue, 'Generate CLI completions')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'boundbook;docs' {
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'boundbook;create' {
            [CompletionResult]::new('-o', '-o', [CompletionResultType]::ParameterName, 'Output BBF file path')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'Output BBF file path')
            [CompletionResult]::new('-O', '-O ', [CompletionResultType]::ParameterName, 'Page order file (format: filename:index)')
            [CompletionResult]::new('--order', '--order', [CompletionResultType]::ParameterName, 'Page order file (format: filename:index)')
            [CompletionResult]::new('-S', '-S ', [CompletionResultType]::ParameterName, 'Sections file (format: Name:Target[:Parent])')
            [CompletionResult]::new('--sections', '--sections', [CompletionResultType]::ParameterName, 'Sections file (format: Name:Target[:Parent])')
            [CompletionResult]::new('-s', '-s', [CompletionResultType]::ParameterName, 'Add section markers (format: Name:Target[:Parent])')
            [CompletionResult]::new('--section', '--section', [CompletionResultType]::ParameterName, 'Add section markers (format: Name:Target[:Parent])')
            [CompletionResult]::new('-m', '-m', [CompletionResultType]::ParameterName, 'Add metadata (format: Key:Value[:Parent])')
            [CompletionResult]::new('--meta', '--meta', [CompletionResultType]::ParameterName, 'Add metadata (format: Key:Value[:Parent])')
            [CompletionResult]::new('-a', '-a', [CompletionResultType]::ParameterName, 'Byte alignment exponent (default: 12 = 4096 bytes)')
            [CompletionResult]::new('--alignment', '--alignment', [CompletionResultType]::ParameterName, 'Byte alignment exponent (default: 12 = 4096 bytes)')
            [CompletionResult]::new('-r', '-r', [CompletionResultType]::ParameterName, 'Ream size exponent (default: 16 = 65536 bytes)')
            [CompletionResult]::new('--ream-size', '--ream-size', [CompletionResultType]::ParameterName, 'Ream size exponent (default: 16 = 65536 bytes)')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Enable variable ream size for smaller files')
            [CompletionResult]::new('--variable-ream-size', '--variable-ream-size', [CompletionResultType]::ParameterName, 'Enable variable ream size for smaller files')
            [CompletionResult]::new('-d', '-d', [CompletionResultType]::ParameterName, 'Auto-detect subdirectories with images and create sections from directory names')
            [CompletionResult]::new('--auto-detect-sections', '--auto-detect-sections', [CompletionResultType]::ParameterName, 'Auto-detect subdirectories with images and create sections from directory names')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'boundbook;info' {
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'boundbook;verify' {
            [CompletionResult]::new('--asset', '--asset', [CompletionResultType]::ParameterName, 'Verify a specific asset by index')
            [CompletionResult]::new('--index-only', '--index-only', [CompletionResultType]::ParameterName, 'Verify only the index hash (faster)')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'boundbook;extract' {
            [CompletionResult]::new('-o', '-o', [CompletionResultType]::ParameterName, 'Output directory for extracted pages')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'Output directory for extracted pages')
            [CompletionResult]::new('--section', '--section', [CompletionResultType]::ParameterName, 'Extract only pages from a specific section')
            [CompletionResult]::new('--until', '--until', [CompletionResultType]::ParameterName, 'Stop extraction when reaching a section matching this string')
            [CompletionResult]::new('--range', '--range', [CompletionResultType]::ParameterName, 'Extract a specific page range (e.g., 1-10 or 5)')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'boundbook;from-cbz' {
            [CompletionResult]::new('-o', '-o', [CompletionResultType]::ParameterName, 'Output BBF file')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'Output BBF file')
            [CompletionResult]::new('-m', '-m', [CompletionResultType]::ParameterName, 'Add metadata (format: Key:Value[:Parent])')
            [CompletionResult]::new('--meta', '--meta', [CompletionResultType]::ParameterName, 'Add metadata (format: Key:Value[:Parent])')
            [CompletionResult]::new('-k', '-k', [CompletionResultType]::ParameterName, 'Keep temporary files for debugging')
            [CompletionResult]::new('--keep-temp', '--keep-temp', [CompletionResultType]::ParameterName, 'Keep temporary files for debugging')
            [CompletionResult]::new('-d', '-d', [CompletionResultType]::ParameterName, 'Process directory of CBZ files as chapters')
            [CompletionResult]::new('--directory-mode', '--directory-mode', [CompletionResultType]::ParameterName, 'Process directory of CBZ files as chapters')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'boundbook;read' {
            [CompletionResult]::new('-W', '-W ', [CompletionResultType]::ParameterName, 'Maximum width in pixels (aspect ratio preserved)')
            [CompletionResult]::new('--max-width', '--max-width', [CompletionResultType]::ParameterName, 'Maximum width in pixels (aspect ratio preserved)')
            [CompletionResult]::new('-H', '-H ', [CompletionResultType]::ParameterName, 'Maximum height in pixels (aspect ratio preserved)')
            [CompletionResult]::new('--max-height', '--max-height', [CompletionResultType]::ParameterName, 'Maximum height in pixels (aspect ratio preserved)')
            [CompletionResult]::new('--max-cols', '--max-cols', [CompletionResultType]::ParameterName, 'Maximum width in terminal columns (overrides max-width if set)')
            [CompletionResult]::new('--max-rows', '--max-rows', [CompletionResultType]::ParameterName, 'Maximum height in terminal rows (overrides max-height if set)')
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Image scaling filter quality')
            [CompletionResult]::new('--filter', '--filter', [CompletionResultType]::ParameterName, 'Image scaling filter quality')
            [CompletionResult]::new('--gif-speed', '--gif-speed', [CompletionResultType]::ParameterName, 'GIF animation frame delay multiplier (1.0 = normal speed)')
            [CompletionResult]::new('-i', '-i', [CompletionResultType]::ParameterName, 'Number of interpolated frames to generate between each GIF frame (0 = disabled)')
            [CompletionResult]::new('--gif-interpolate', '--gif-interpolate', [CompletionResultType]::ParameterName, 'Number of interpolated frames to generate between each GIF frame (0 = disabled)')
            [CompletionResult]::new('-m', '-m', [CompletionResultType]::ParameterName, 'Frame interpolation algorithm')
            [CompletionResult]::new('--interpolation-method', '--interpolation-method', [CompletionResultType]::ParameterName, 'Frame interpolation algorithm')
            [CompletionResult]::new('--sidebar-width', '--sidebar-width', [CompletionResultType]::ParameterName, 'Sidebar width in columns')
            [CompletionResult]::new('--slideshow-delay', '--slideshow-delay', [CompletionResultType]::ParameterName, 'Slideshow auto-advance delay in seconds')
            [CompletionResult]::new('-P', '-P ', [CompletionResultType]::ParameterName, 'Pre-render all pages before reading (uses more memory but smoother navigation)')
            [CompletionResult]::new('--prerender', '--prerender', [CompletionResultType]::ParameterName, 'Pre-render all pages before reading (uses more memory but smoother navigation)')
            [CompletionResult]::new('-g', '-g', [CompletionResultType]::ParameterName, 'Enable GIF animation playback')
            [CompletionResult]::new('--enable-gif-animation', '--enable-gif-animation', [CompletionResultType]::ParameterName, 'Enable GIF animation playback')
            [CompletionResult]::new('-l', '-l', [CompletionResultType]::ParameterName, 'Loop GIFs infinitely')
            [CompletionResult]::new('--gif-loop', '--gif-loop', [CompletionResultType]::ParameterName, 'Loop GIFs infinitely')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            break
        }
        'boundbook;complete' {
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'boundbook;help' {
            [CompletionResult]::new('docs', 'docs', [CompletionResultType]::ParameterValue, 'Print help')
            [CompletionResult]::new('create', 'create', [CompletionResultType]::ParameterValue, 'Create a BBF file from images')
            [CompletionResult]::new('info', 'info', [CompletionResultType]::ParameterValue, 'Display BBF file information')
            [CompletionResult]::new('verify', 'verify', [CompletionResultType]::ParameterValue, 'Verify BBF file integrity')
            [CompletionResult]::new('extract', 'extract', [CompletionResultType]::ParameterValue, 'Extract pages from a BBF file')
            [CompletionResult]::new('from-cbz', 'from-cbz', [CompletionResultType]::ParameterValue, 'Convert CBZ archive to BBF format')
            [CompletionResult]::new('read', 'read', [CompletionResultType]::ParameterValue, 'Read a BBF file in the terminal')
            [CompletionResult]::new('complete', 'complete', [CompletionResultType]::ParameterValue, 'Generate CLI completions')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'boundbook;help;docs' {
            break
        }
        'boundbook;help;create' {
            break
        }
        'boundbook;help;info' {
            break
        }
        'boundbook;help;verify' {
            break
        }
        'boundbook;help;extract' {
            break
        }
        'boundbook;help;from-cbz' {
            break
        }
        'boundbook;help;read' {
            break
        }
        'boundbook;help;complete' {
            break
        }
        'boundbook;help;help' {
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
