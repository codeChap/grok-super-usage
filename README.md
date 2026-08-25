# Grokbar

SuperGrok weekly usage and xAI API invoice spend for the [Omarchy](https://omarchy.org/) bar.

Click the chip for Build / Chat / Imagine, reset time, and this cycle’s API bill. A cog in the panel opens settings.

This is **subscription quota + Management API postpaid spend**. It is not xAI prepaid Management balance by itself, and it is not Cursor usage.

## Install

Needs Omarchy Quattro (Quickshell plugins), Rust (`cargo` / `rustc`), and Python is not required.

```sh
omarchy plugin add https://github.com/codeChap/omarchy-grokbar.git --enable
cd ~/.config/omarchy/plugins/codechap.grokbar
./install.sh
omarchy restart shell
```

`install.sh` builds the `grokbar` scanner and places it next to the QML. `omarchy plugin add` only clones the repo; the binary is not prebuilt.

### SuperGrok weekly

Sign in with the official Grok Build CLI:

```sh
grok login
```

Weekly percent comes from grok.com `GetGrokCreditsConfig` using `~/.grok/auth.json`.

### API billing (optional)

A **team-scoped Management API key** from [console.x.ai](https://console.x.ai) (not an inference key). Either:

- put the key in `~/dev/XAI-MGMT-KEY.txt` (`chmod 600`), or
- paste the key or a path in **Settings** (cog), or
- `export XAI_MANAGEMENT_KEY=...`

Spend is the current postpaid invoice preview (`amountAfterVat`).

## Usage

- Left click the bar chip: open / close the panel
- Right click: refresh
- Cog: settings (what the bar shows, pace warning, management key)
- Escape: close settings, then the panel

The panel always shows weekly + API. Bar visibility of each is optional.

## Configure

```sh
omarchy bar move codechap.grokbar --section right
```

| Setting | Default | Bar effect |
| --- | --- | --- |
| Show weekly usage | on | `%` and reset (`6d`) |
| Show API billing | on | `$12.34 API` |
| Pace warning | **off** | red when weekly use is ahead of even-burn |

## Remove

```sh
omarchy plugin disable codechap.grokbar
omarchy plugin remove codechap.grokbar --yes
```

Removal deletes the cloned plugin folder. It does not change `~/.grok/auth.json` or your management key file.

## Privacy

- SuperGrok: reads local `~/.grok/auth.json`, may refresh the OIDC token in that file.
- API billing: reads a management key from a file, env, or the settings field. Prefer a `chmod 600` file; a pasted key is stored in `~/.config/omarchy/shell.json`.
- Tokens are not logged. This plugin does not send credentials to third parties; it only calls grok.com and `management-api.x.ai`.

## Develop

```sh
cargo test
omarchy plugin validate .
qmllint -I "$OMARCHY_PATH/shell" *.qml
./install.sh
```

## Credits

Panel layout started from [rlimberger/grokbar-omarchy](https://github.com/rlimberger/grokbar-omarchy) (MIT). Scanner is Rust.

## License

MIT. See [LICENSE](LICENSE).
