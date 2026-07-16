complete -c dexdeck -f
complete -c dexdeck -n '__fish_use_subcommand' -a 'init doctor project modules variants devices emulators build install launch run rerun reinstall clean-reinstall stop uninstall clear-data test logs gradle emulator command version'
complete -c dexdeck -l project -r
complete -c dexdeck -l sdk -r
complete -c dexdeck -l module -r
complete -c dexdeck -l variant -r
complete -c dexdeck -l device -r
complete -c dexdeck -l profile -r
complete -c dexdeck -l format -r -a 'human json jsonl'
complete -c dexdeck -l no-color
complete -c dexdeck -l ascii
