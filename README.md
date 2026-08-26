# Grok Super Usage

SuperGrok weekly usage and xAI API invoice spend for the [Omarchy](https://omarchy.org/) bar.

Click the chip for Build / Chat / Imagine, reset time, and this cycle’s API bill. A cog in the panel opens settings.

<img src="https://raw.githubusercontent.com/codeChap/grok-super-usage/main/preview.png" alt="Grok Super Usage panel: weekly percent, product slices, and API bill" width="404" />

This is **subscription quota + Management API postpaid spend**. It is not xAI prepaid Management balance by itself, and it is not Cursor usage.

## Pass this to your AI

Copy the block below into Grok, Claude, or whatever agent you use on this machine.

````
Install the Omarchy bar plugin Grok Super Usage from https://github.com/codeChap/grok-super-usage.git

This is SuperGrok weekly quota plus optional xAI Management API invoice spend. It is not Cursor usage and not prepaid Management balance.

1. If codechap.grokbar is installed, disable and remove it.
2. Require Omarchy Quattro and Rust (cargo / rustc). Python is not required.
3. Run:
   omarchy plugin add https://github.com/codeChap/grok-super-usage.git --enable
   cd ~/.config/omarchy/plugins/codechap.grok-super-usage
   ./install.sh
   omarchy restart shell
4. Plugin id is codechap.grok-super-usage. Put it on the right of the bar if it is not already there:
   omarchy bar move codechap.grok-super-usage --section right
5. Weekly percent needs `grok login` and ~/.grok/auth.json.
6. API dollars are optional. Use a team-scoped Management API key from console.x.ai in a chmod 600 file (default ~/dev/XAI-MGMT-KEY.txt) or Settings. Never put the key on the command line or in shell.json. Store a path only.
7. Do not skip ./install.sh. plugin add only clones. The scanner binary is grok-super-usage next to the QML.
````

## Install

Needs Omarchy Quattro (Quickshell plugins), Rust (`cargo` / `rustc`), and Python is not required.

```sh
omarchy plugin add https://github.com/codeChap/grok-super-usage.git --enable
cd ~/.config/omarchy/plugins/codechap.grok-super-usage
./install.sh
omarchy restart shell
```

`install.sh` builds the `grok-super-usage` scanner and places it next to the QML. `omarchy plugin add` only clones the repo; the binary is not prebuilt.

If you still have the old `codechap.grokbar` plugin, remove it first:

```sh
omarchy plugin disable codechap.grokbar
omarchy plugin remove codechap.grokbar --yes
```

### SuperGrok weekly

Sign in with the official Grok Build CLI:

```sh
grok login
```

Weekly percent comes from grok.com `GetGrokCreditsConfig` using `~/.grok/auth.json`.

### API billing (optional)

A **team-scoped Management API key** from [console.x.ai](https://console.x.ai) (not an inference key). Prefer a `chmod 600` file:

- `~/dev/XAI-MGMT-KEY.txt`, or
- a path in **Settings** (cog), or
- `export XAI_MANAGEMENT_KEY=...` (the key itself, not on the command line)

Pasting a key in Settings writes `management.key` next to the plugin (mode 600) and stores **that path** only. The key is never put on `ps` argv and is not saved in `shell.json`.

Spend is the current postpaid invoice preview (`amountAfterVat`).

## Usage

- Left click the bar chip: open / close the panel
- Right click: refresh
- Cog: settings (what the bar shows, pace warning, management key)
- Escape: close settings, then the panel

The panel always shows weekly + API. Bar visibility of each is optional.

## Configure

```sh
omarchy bar move codechap.grok-super-usage --section right
```

| Setting | Default | Bar effect |
| --- | --- | --- |
| Show weekly usage | on | `%` and reset (`6d`) |
| Show API billing | on | `$12.34 API` |
| Pace warning | **off** | red when weekly use is ahead of even-burn |

## Remove

```sh
omarchy plugin disable codechap.grok-super-usage
omarchy plugin remove codechap.grok-super-usage --yes
```

Removal deletes the cloned plugin folder. It does not change `~/.grok/auth.json` or your management key file.

## Privacy

- SuperGrok: reads local `~/.grok/auth.json`, may refresh the OIDC token in that file (flock + merge so it does not clobber a concurrent `grok login`).
- API billing: reads a management key from a chmod 600 file or `XAI_MANAGEMENT_KEY`. Settings store a **path**, never the key.
- Tokens are not logged. This plugin does not send credentials to third parties; it only calls grok.com and `management-api.x.ai`.

## Develop

```sh
cargo test
omarchy plugin validate .
qmllint -I "$OMARCHY_PATH/shell" *.qml
./install.sh
```

GitHub Actions (the **Actions** tab) runs `cargo test --locked` and a release build on every push to `main` and on every pull request. That is CI. Same commands as above, on a clean Ubuntu machine GitHub starts for you. QML lint stays local because it needs the Omarchy shell.

## Credits

Panel layout started from [rlimberger/grokbar-omarchy](https://github.com/rlimberger/grokbar-omarchy) (MIT). Scanner is Rust.

## License

MIT. See [LICENSE](LICENSE).
