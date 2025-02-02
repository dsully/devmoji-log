# devmoji-log

Display Git commit history with [conventional commit](https://www.conventionalcommits.org/) types and [gitmoji](https://gitmoji.dev) support.

## Features

- Shows recent Git commits with clickable commit hashes
- Automatically detects and displays appropriate emojis based on conventional commit types
- Relative timestamps for commits

## Installation

```bash
cargo install --git https://github.com/dsully/devmoji-log
```

## Usage

```bash
# Show the last 5 commits (default)
devmoji-log

# Show the last N commits
devmoji-log -c 10
```

## Fish Shell Integration:

Create a function in your fish config, which calls `devmoji-log` when entering a Git repository.

```fish
function auto_pwd --on-variable PWD

    # Only run when changing to the root of the repository.
    if test -d "$PWD/.git"
        devmoji-log
    end
end
```

## Example Output

```
  * abc123f feat: ✨ Add new feature (2 days ago)
  * def456a fix: 🐛 Fix critical bug (5 hours ago)
  * ghi789b docs: 📚 Update documentation (1 hour ago)
```

## Inspiration

Folke's [devmoji](https://github.com/folke/devmoji)
