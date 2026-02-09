# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_boundbook_global_optspecs
	string join \n h/help V/version
end

function __fish_boundbook_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_boundbook_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_boundbook_using_subcommand
	set -l cmd (__fish_boundbook_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c boundbook -n "__fish_boundbook_needs_command" -s h -l help -d 'Print help'
complete -c boundbook -n "__fish_boundbook_needs_command" -s V -l version -d 'Print version'
complete -c boundbook -n "__fish_boundbook_needs_command" -f -a "docs" -d 'Print help'
complete -c boundbook -n "__fish_boundbook_needs_command" -f -a "create" -d 'Create a BBF file from images'
complete -c boundbook -n "__fish_boundbook_needs_command" -f -a "info" -d 'Display BBF file information'
complete -c boundbook -n "__fish_boundbook_needs_command" -f -a "verify" -d 'Verify BBF file integrity'
complete -c boundbook -n "__fish_boundbook_needs_command" -f -a "extract" -d 'Extract pages from a BBF file'
complete -c boundbook -n "__fish_boundbook_needs_command" -f -a "from-cbz" -d 'Convert CBZ archive to BBF format'
complete -c boundbook -n "__fish_boundbook_needs_command" -f -a "read" -d 'Read a BBF file in the terminal'
complete -c boundbook -n "__fish_boundbook_needs_command" -f -a "complete" -d 'Generate CLI completions'
complete -c boundbook -n "__fish_boundbook_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c boundbook -n "__fish_boundbook_using_subcommand docs" -s h -l help -d 'Print help'
complete -c boundbook -n "__fish_boundbook_using_subcommand create" -s o -l output -d 'Output BBF file path' -r -F
complete -c boundbook -n "__fish_boundbook_using_subcommand create" -s O -l order -d 'Page order file (format: filename:index)' -r -F
complete -c boundbook -n "__fish_boundbook_using_subcommand create" -s S -l sections -d 'Sections file (format: Name:Target[:Parent])' -r -F
complete -c boundbook -n "__fish_boundbook_using_subcommand create" -s s -l section -d 'Add section markers (format: Name:Target[:Parent])' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand create" -s m -l meta -d 'Add metadata (format: Key:Value[:Parent])' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand create" -s a -l alignment -d 'Byte alignment exponent (default: 12 = 4096 bytes)' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand create" -s r -l ream-size -d 'Ream size exponent (default: 16 = 65536 bytes)' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand create" -s v -l variable-ream-size -d 'Enable variable ream size for smaller files'
complete -c boundbook -n "__fish_boundbook_using_subcommand create" -s d -l auto-detect-sections -d 'Auto-detect subdirectories with images and create sections from directory names'
complete -c boundbook -n "__fish_boundbook_using_subcommand create" -s h -l help -d 'Print help'
complete -c boundbook -n "__fish_boundbook_using_subcommand info" -s h -l help -d 'Print help'
complete -c boundbook -n "__fish_boundbook_using_subcommand verify" -l asset -d 'Verify a specific asset by index' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand verify" -l index-only -d 'Verify only the index hash (faster)'
complete -c boundbook -n "__fish_boundbook_using_subcommand verify" -s h -l help -d 'Print help'
complete -c boundbook -n "__fish_boundbook_using_subcommand extract" -s o -l output -d 'Output directory for extracted pages' -r -F
complete -c boundbook -n "__fish_boundbook_using_subcommand extract" -l section -d 'Extract only pages from a specific section' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand extract" -l until -d 'Stop extraction when reaching a section matching this string' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand extract" -l range -d 'Extract a specific page range (e.g., 1-10 or 5)' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand extract" -s h -l help -d 'Print help'
complete -c boundbook -n "__fish_boundbook_using_subcommand from-cbz" -s o -l output -d 'Output BBF file' -r -F
complete -c boundbook -n "__fish_boundbook_using_subcommand from-cbz" -s m -l meta -d 'Add metadata (format: Key:Value[:Parent])' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand from-cbz" -s k -l keep-temp -d 'Keep temporary files for debugging'
complete -c boundbook -n "__fish_boundbook_using_subcommand from-cbz" -s d -l directory-mode -d 'Process directory of CBZ files as chapters'
complete -c boundbook -n "__fish_boundbook_using_subcommand from-cbz" -s h -l help -d 'Print help'
complete -c boundbook -n "__fish_boundbook_using_subcommand read" -s W -l max-width -d 'Maximum width in pixels (aspect ratio preserved)' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand read" -s H -l max-height -d 'Maximum height in pixels (aspect ratio preserved)' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand read" -l max-cols -d 'Maximum width in terminal columns (overrides max-width if set)' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand read" -l max-rows -d 'Maximum height in terminal rows (overrides max-height if set)' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand read" -s f -l filter -d 'Image scaling filter quality' -r -f -a "nearest\t''
triangle\t''
catmull-rom\t''
gaussian\t''
lanczos3\t''"
complete -c boundbook -n "__fish_boundbook_using_subcommand read" -l gif-speed -d 'GIF animation frame delay multiplier (1.0 = normal speed)' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand read" -s i -l gif-interpolate -d 'Number of interpolated frames to generate between each GIF frame (0 = disabled)' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand read" -s m -l interpolation-method -d 'Frame interpolation algorithm' -r -f -a "blend\t'Simple linear blending (fastest)'
smooth\t'Weighted blending with ease-in/ease-out'
cosine\t'Cosine interpolation for smoother transitions'
cubic\t'Cubic hermite spline interpolation'
perlin\t'Perlin smoothstep (quintic hermite)'
exponential\t'Exponential ease-in-out'
optical-flow-sparse\t'Optical flow based (Lucas-Kanade sparse)'
motion-compensated\t'Motion-compensated blending (simplified Horn-Schunck)'
catmull-rom\t'Catmull-Rom spline (requires 4 frames, falls back to cubic)'"
complete -c boundbook -n "__fish_boundbook_using_subcommand read" -l sidebar-width -d 'Sidebar width in columns' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand read" -l slideshow-delay -d 'Slideshow auto-advance delay in seconds' -r
complete -c boundbook -n "__fish_boundbook_using_subcommand read" -s P -l prerender -d 'Pre-render all pages before reading (uses more memory but smoother navigation)'
complete -c boundbook -n "__fish_boundbook_using_subcommand read" -s g -l enable-gif-animation -d 'Enable GIF animation playback'
complete -c boundbook -n "__fish_boundbook_using_subcommand read" -s l -l gif-loop -d 'Loop GIFs infinitely'
complete -c boundbook -n "__fish_boundbook_using_subcommand read" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c boundbook -n "__fish_boundbook_using_subcommand complete" -s h -l help -d 'Print help'
complete -c boundbook -n "__fish_boundbook_using_subcommand help; and not __fish_seen_subcommand_from docs create info verify extract from-cbz read complete help" -f -a "docs" -d 'Print help'
complete -c boundbook -n "__fish_boundbook_using_subcommand help; and not __fish_seen_subcommand_from docs create info verify extract from-cbz read complete help" -f -a "create" -d 'Create a BBF file from images'
complete -c boundbook -n "__fish_boundbook_using_subcommand help; and not __fish_seen_subcommand_from docs create info verify extract from-cbz read complete help" -f -a "info" -d 'Display BBF file information'
complete -c boundbook -n "__fish_boundbook_using_subcommand help; and not __fish_seen_subcommand_from docs create info verify extract from-cbz read complete help" -f -a "verify" -d 'Verify BBF file integrity'
complete -c boundbook -n "__fish_boundbook_using_subcommand help; and not __fish_seen_subcommand_from docs create info verify extract from-cbz read complete help" -f -a "extract" -d 'Extract pages from a BBF file'
complete -c boundbook -n "__fish_boundbook_using_subcommand help; and not __fish_seen_subcommand_from docs create info verify extract from-cbz read complete help" -f -a "from-cbz" -d 'Convert CBZ archive to BBF format'
complete -c boundbook -n "__fish_boundbook_using_subcommand help; and not __fish_seen_subcommand_from docs create info verify extract from-cbz read complete help" -f -a "read" -d 'Read a BBF file in the terminal'
complete -c boundbook -n "__fish_boundbook_using_subcommand help; and not __fish_seen_subcommand_from docs create info verify extract from-cbz read complete help" -f -a "complete" -d 'Generate CLI completions'
complete -c boundbook -n "__fish_boundbook_using_subcommand help; and not __fish_seen_subcommand_from docs create info verify extract from-cbz read complete help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
