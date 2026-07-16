_dexdeck() {
  local commands="init doctor project modules variants devices emulators build install launch run rerun reinstall clean-reinstall stop uninstall clear-data test logs gradle emulator command version"
  local options="--project --sdk --module --variant --device --profile --format --gradle-arg --no-color --ascii --debug-log --config --yes --help --version"
  COMPREPLY=($(compgen -W "$commands $options" -- "${COMP_WORDS[COMP_CWORD]}"))
}
complete -F _dexdeck dexdeck
