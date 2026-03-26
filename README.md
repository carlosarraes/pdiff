# pdiff

Terminal diff reviewer with vim motions. Select code, comment, export.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/carlosarraes/pdiff/main/install.sh | bash
```

Or with cargo:
```bash
cargo install --git https://github.com/carlosarraes/pdiff
```

## Usage

```bash
git diff | pdiff
git diff --cached | pdiff
git diff main...HEAD | pdiff
pdiff --input diff.patch
```

## Keys

| Key | Action |
|-----|--------|
| `j/k` | Navigate lines |
| `h/l` | Switch focus: OLD / NEW |
| `gg/G` | Jump to top / bottom |
| `Ctrl-d/u` | Half-page scroll (centered) |
| `]/[` | Next / previous hunk |
| `H/L` | Next / previous file |
| `V` | Visual line select |
| `c` | Comment (single line or visual selection) |
| `/` | Search, `n/N` for next/prev |
| `e` | Toggle file list |
| `E` | Toggle expanded comments |
| `F` | Focus mode (single panel, full width) |
| `Tab` | Toggle layout |
| `q` | Quit |

### Comment popup

| Key | Action |
|-----|--------|
| Type | Insert text |
| `Enter` | Submit comment |
| `Esc` | Switch to normal mode |
| `a/i` | Back to insert mode |
| `Esc` (normal) | Cancel comment |

## Output

On quit, pdiff prompts to save comments to `pdiff-review.md`:

```markdown
## Review Comments

### src/auth.rs:10-12(new)
> +    token.len() > 0

Should use proper JWT validation.
```

Flags: `--output <path>` to auto-write, `--stdout` to print to stdout.

## Pi Integration

```bash
pdiff install pi    # Install /pdiff command for pi
pdiff uninstall pi  # Remove it
```

In pi, run `/pdiff` to select a review target (staged changes, base branch, specific commit). Comments flow back to pi's prompt.
