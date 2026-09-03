<div align="center">
  <h1>rhisper <i>/ˈrɪspər/</i></h1>
  <img src="logo.png" alt="rhisper logo" width="300">
  <br><br>
  <a href="https://github.com/lv10/rhisper/actions/workflows/ci.yml"><img src="https://github.com/lv10/rhisper/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://buymeacoffee.com/luisvillamg"><img src="https://img.shields.io/badge/Buy%20me%20a%20coffee-donate-ffdd00?logo=buy-me-a-coffee&logoColor=black" alt="Buy me a coffee"></a>
  <br><br>
</div>

Dictation at cursor for Linux and macOS. A Rust rewrite based on the original [xhisper](https://github.com/imaginalnika/xhisper) project.

## Installation

### Runtime dependencies

On Linux, rhisper needs `pipewire` (for `pw-record`) and `ffmpeg` at runtime. On macOS, it needs `sox` (for `rec`) and `ffmpeg`. Everything else (HTTP, JSON, clipboard) is built into the binary.

<details>
<summary>Arch Linux / Manjaro</summary>
<pre><code>sudo pacman -S pipewire ffmpeg</code></pre>
</details>

<details>
<summary>Debian / Ubuntu / Linux Mint</summary>
<pre><code>sudo apt update
sudo apt install pipewire ffmpeg</code></pre>
</details>

<details>
<summary>Fedora / RHEL / AlmaLinux / Rocky</summary>
Fedora's base repos don't ship <code>ffmpeg</code> (licensing) — enable <a href="https://rpmfusion.org/Configuration">RPM Fusion</a> first:
<pre><code>sudo dnf install -y https://download1.rpmfusion.org/free/fedora/rpmfusion-free-release-$(rpm -E %fedora).noarch.rpm
sudo dnf install -y pipewire-utils ffmpeg</code></pre>
</details>

<details>
<summary>macOS</summary>
<pre><code>brew install sox ffmpeg</code></pre>
rhisper also needs Accessibility permission to type at your cursor (System Settings → Privacy & Security → Accessibility) and Microphone permission for <code>rec</code> to capture audio — <code>rhisper --setup</code> checks and prompts for these.
</details>

### Install the package

<details>
<summary>Arch Linux (AUR)</summary>
<pre><code>yay -S rhisper
# or
paru -S rhisper</code></pre>
or manually from <code>packaging/PKGBUILD</code>:
<pre><code>git clone --depth 1 https://github.com/lv10/rhisper.git
cd rhisper/packaging && makepkg -si</code></pre>
Note: this installs via an AUR helper or <code>makepkg</code>, not bare <code>pacman -S</code> — pacman itself has no native AUR support, which is true of every AUR package, not just this one.
</details>

<details>
<summary>Debian / Ubuntu (APT repository)</summary>
<pre><code>curl -1sLf 'https://dl.cloudsmith.io/public/lv10-labs/rhisper/gpg.key' | sudo gpg --dearmor -o /usr/share/keyrings/rhisper-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/rhisper-archive-keyring.gpg] https://dl.cloudsmith.io/public/lv10-labs/rhisper/deb/ubuntu jammy main" | sudo tee /etc/apt/sources.list.d/rhisper.list
sudo apt update && sudo apt install rhisper</code></pre>
Or download the <code>.deb</code> directly from the <a href="https://github.com/lv10/rhisper/releases">latest release</a> and <code>sudo apt install ./rhisper_*.deb</code>.
</details>

<details>
<summary>Fedora / RHEL / AlmaLinux / Rocky</summary>
Download the <code>.rpm</code> from the <a href="https://github.com/lv10/rhisper/releases">latest release</a>, then:
<pre><code>sudo dnf install ./rhisper-*.rpm</code></pre>
</details>

<details>
<summary>macOS (Homebrew)</summary>
<pre><code>brew tap lv10/rhisper
brew install rhisper</code></pre>
</details>

<details>
<summary>cargo (any distro with Rust installed)</summary>
<pre><code>cargo install rhisper</code></pre>
On Linux, <code>cargo install</code> only builds and places the two binaries on your <code>$PATH</code> — it has no post-install hook, so it can't install the udev rule the .deb/.rpm/AUR packages ship. Without it, the daemon can't open <code>/dev/uinput</code> unless you're already in the <code>input</code> group. After installing, either run:
<pre><code>sudo usermod -aG input $USER   # then log out and back in</code></pre>
or install the udev rule yourself:
<pre><code>curl -sL https://raw.githubusercontent.com/lv10/rhisper/main/packaging/rhisper-uinput.rules | sudo tee /usr/lib/udev/rules.d/60-rhisper-uinput.rules
sudo udevadm control --reload-rules && sudo udevadm trigger --subsystem-match=misc --attr-match=name=uinput</code></pre>
On macOS there's no daemon/udev rule at all — just grant Accessibility and Microphone permission (<code>rhisper --setup</code> checks and prompts for these).
</details>

<details>
<summary>Build from source (any distro)</summary>
Requires the Rust toolchain (<a href="https://rustup.rs">rustup.rs</a>) and <code>nasm</code> (a build-time dependency of the TLS stack):
<pre><code>git clone --depth 1 https://github.com/lv10/rhisper.git
cd rhisper
cargo build --release
sudo install -Dm755 target/release/rhisper /usr/local/bin/rhisper</code></pre>
On Linux, also install the uinput daemon/client binary and its udev rule (macOS has no equivalent — <code>rhispertool</code> is a Linux-only stub there, see <a href="#usage">Usage</a>):
<pre><code>sudo install -Dm755 target/release/rhispertool /usr/local/bin/rhispertool
sudo ln -sf rhispertool /usr/local/bin/rhispertoold
sudo install -Dm644 packaging/rhisper-uinput.rules /usr/lib/udev/rules.d/60-rhisper-uinput.rules
sudo udevadm control --reload-rules && sudo udevadm trigger --subsystem-match=misc --attr-match=name=uinput</code></pre>
</details>

Linux packages install a udev rule granting access to `/dev/uinput` automatically — no group membership change or re-login needed. If you built from source without the udev rule (or on a system without udev-tag-aware session management), fall back to:
```sh
sudo usermod -aG input $USER
```
then **log out and log back in** (restart is safer) for the group change to take effect. Check with `groups` — you should see `input` in the output.

### Setup

1. **Get a Groq API key** from [console.groq.com](https://console.groq.com) (free tier available) and add to `~/.env`:
```sh
GROQ_API_KEY=<your_API_key>
```
(Or run `rhisper --setup` for an interactive prompt plus a platform permission/dependency check — `/dev/uinput` on Linux, Accessibility on macOS.)

2. Bind `rhisper` binary to your favorite key:

<details>
<summary>keyd</summary>

```ini
[main]
capslock = layer(dictate)

[dictate:C]
d = macro(rhisper)
```
</details>

<details>
<summary>sxhkd</summary>

```
super + d
    rhisper
```
</details>

<details>
<summary>i3 / sway</summary>

```
bindsym $mod+d exec rhisper
```
</details>

<details>
<summary>Hyprland</summary>

```
bind = $mainMod, D, exec, rhisper
```
</details>

<details>
<summary>macOS (skhd)</summary>

macOS has no bundled hotkey daemon; <a href="https://github.com/koekeishiya/skhd">skhd</a> is the closest widely-used equivalent (also needs Accessibility permission, granted separately from rhisper's own):

```sh
brew install skhd
```

```
# ~/.config/skhd/skhdrc
cmd + shift - d : rhisper
```

```sh
skhd --start-service
```
</details>

<details>
<summary>Gnome</summary>

```sh
# In your terminal:

name="rhisper"
binding="<CTRL><SHIFT>X"
action="/usr/local/bin/rhisper"

media_keys=org.gnome.settings-daemon.plugins.media-keys
custom_kbd=org.gnome.settings-daemon.plugins.media-keys.custom-keybinding
kbd_path=/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/$name/
new_bindings=`gsettings get $media_keys custom-keybindings | sed -e"s>'\]>','$kbd_path']>"| sed -e"s>@as \[\]>['$kbd_path']>"`
gsettings set $media_keys custom-keybindings "$new_bindings"
gsettings set $custom_kbd:$kbd_path name "$name"
gsettings set $custom_kbd:$kbd_path binding "$binding"
gsettings set $custom_kbd:$kbd_path command "$action"
```
</details>

---

## Usage

Simply run `rhisper` twice (via your favorite keybinding):
- **First run**: Starts recording
- **Second run**: Stops and transcribes

The transcription will be typed at your cursor position.

**View logs:**
```sh
rhisper --log
```

**Choosing a microphone:**

By default rhisper records from the system's default input. With several microphones connected you can name the one to dictate into:

```sh
rhisper --list-devices
```

Then put the device in `~/.config/rhisper/rhisperrc`:

```
audio-device : "alsa_input.usb-Anua_Mic_CM_900_Anua_Mic_CM_900-00.mono-fallback"
```

On Linux this is a PipeWire target — a node name or a numeric node id. On macOS it is a CoreAudio device name (sox's `AUDIODEV`), and `--list-devices` shows what `system_profiler` reports. Leave it empty to keep following the system default.

If that microphone is not plugged in when you dictate, rhisper records from the system default instead and notes it in `rhisper --log`. To be told instead of quietly recording from somewhere else:

```
audio-device-fallback : no
```

This check exists because `pw-record` does not fail on an unknown target — PipeWire links the stream to the default source without complaining, so a missing microphone otherwise looks exactly like a working one. macOS cannot enumerate devices this way, so the setting is Linux-only.

**Status feedback while dictating:**

rhisper shows what it is doing — recording, transcribing, or that it heard nothing. By default (`placeholders : auto`) that is a desktop notification whenever `paste-mode` is one of the clipboard modes, and inline text otherwise.

Inline means the status is typed into the field you are dictating into and then deleted again with one backspace per character. That is fine in `type` mode, where the deletion is exactly as reliable as the typing was, but in clipboard modes the text arrives in one paste into a field that may autocomplete or reformat it, and the backspace count becomes a guess. Notifications keep the target field untouched.

Force one or the other, or turn the status off entirely:

```
placeholders : notify      # auto | inline | notify | off
```

`notify` needs `notify-send` (libnotify) to post and `gdbus` (glib) to dismiss; both ship with any desktop that has notifications. macOS has no `notify-send`, so `auto` resolves to inline there. The three status texts are configurable — `placeholder-recording`, `placeholder-transcribing`, `placeholder-silent` (emoji work fine).

They can also name the microphone in use, via `${device}`:

```
placeholder-recording : "🎤 ${device}"      # -> 🎤 Anua Mic CM 900 Mono
```

That is the human-readable description of the source actually being recorded from — with `audio-device` set it is that device, with `audio-device` empty it is whatever the system default currently is, so the status answers "which microphone is live?" before you start talking. Where devices cannot be enumerated (macOS), it falls back to the configured value, or to `system default` when there is none. Unknown `${names}` are left in place rather than silently dropped.

**Non-QWERTY layouts (Linux only):**

Linux types by simulating physical key positions, so a non-QWERTY layout (e.g. Dvorak, International) needs an input switch key to QWERTY (e.g. rightalt) set up first. Then instead of binding to `rhisper`, bind to:
```sh
rhisper --<your-input-switch-key>
```

**Available input switch keys:** `--leftalt`, `--rightalt`, `--leftctrl`, `--rightctrl`, `--leftshift`, `--rightshift`, `--super` (on macOS, `--super` wraps with Cmd instead — there's no physical Super/Windows key)

Key chords (like ctrl-space) not available yet. macOS doesn't need any of this: it types via direct Unicode injection regardless of the active keyboard layout, so there's nothing to switch.

**Keyboard layout for typed symbols (Linux only):**

ASCII letters and digits are typed by physical key position, but punctuation symbols differ between layouts. If you use a Danish or Spanish keyboard layout and get wrong characters (e.g. on Danish, `'` comes out as `ø`; on Spanish, `~` comes out as `ª`), set the layout in `~/.config/rhisper/rhisperrc`:

```
keyboard-layout : dk
```

Supported layouts: `us`, `dk`, `es`. The layout is read by the daemon at startup; the daemon is restarted automatically when the setting changes. Ignored on macOS (see above).

---

## Configuration

Configuration is read from `~/.config/rhisper/rhisperrc`. A default one is created automatically the first time you run `rhisper` — no setup step required.

To view or reset your configuration:
```sh
rhisper --config                                  # print current config
cp /usr/share/rhisper/rhisperrc.default \
   ~/.config/rhisper/rhisperrc                    # reset to defaults
```

| Option | Default | Description |
|--------|---------|-------------|
| `provider` | `groq` | Transcription provider: `groq` (reads `GROQ_API_KEY`), `openai` (reads `OPENAI_API_KEY`), or `custom` (reads `RHISPER_API_KEY`, sends to `api-base-url`) |
| `api-base-url` | _(empty)_ | Base URL for `provider: custom` — any OpenAI-compatible `/audio/transcriptions` endpoint |
| `model` | _(empty)_ | Overrides the provider's default model (e.g. `whisper-1`, `gpt-4o-transcribe`). Empty = provider default |
| `long-recording-threshold` | `1000` | Duration in seconds above which Groq's larger `whisper-large-v3` model is used instead of `whisper-large-v3-turbo` |
| `transcription-prompt` | _(empty)_ | Context hint passed to Whisper to improve accuracy (e.g. common words, names, or domain vocabulary) |
| `language` | _(empty)_ | ISO-639-1 code (e.g. `de`, `en`, `fr`) to force the transcription language — leave empty to let Whisper auto-detect |
| `paste-mode` | `type` | `type` (layout-sensitive, keeps clipboard), `clipboard` (always paste via clipboard, overwrites clipboard), or `clipboard-restore` (like `clipboard`, but saves and restores previous clipboard content) |
| `non-ascii-initial-delay` | `0.15` | Seconds to wait before pasting the first non-ASCII clipboard chunk — increase if the first character is wrong |
| `non-ascii-default-delay` | `0.025` | Seconds to wait before subsequent non-ASCII clipboard chunks |
| `keyboard-layout` | `us` | **Linux only.** Keyboard layout used when typing ASCII characters (`us`, `dk`, or `es`) — see [Keyboard layout for typed symbols](#usage). Ignored on macOS, which types via direct Unicode injection |
| `silence-threshold` | `-50` | Max volume in dB below which audio counts as quiet (e.g. `-50` means anything quieter is discarded) |
| `audio-device-fallback` | `yes` | When `audio-device` is set but absent: `yes` records from the system default instead, `no` refuses to record. **Linux only** |
| `audio-device` | *(empty)* | Explicit capture source; empty follows the system default. Linux: a PipeWire node name or id; macOS: a CoreAudio device name. List them with `rhisper --list-devices` |
| `placeholders` | `auto` | How the recording/transcribing status is shown: `auto` (notify when `paste-mode` is a clipboard one and `notify-send` exists, else inline), `inline`, `notify`, or `off` |
| `placeholder-recording` | `(recording...)` | Status shown while recording. May contain `${device}` — the name of the microphone in use |
| `placeholder-transcribing` | `(transcribing...)` | Status shown while the transcription request is in flight |
| `placeholder-silent` | `(no sound detected)` | Status shown when the recording contained no speech |
| `min-speech-seconds` | `0.3` | Minimum contiguous stretch of audio above `silence-threshold` required anywhere in the recording to count as real speech, regardless of how long you pause before/after |

## Troubleshooting

**Terminal Applications**: Clipboard paste uses Ctrl+V, which doesn't work in terminal emulators (they require Ctrl+Shift+V). Temporary workaround is to remap Ctrl+V to paste in your terminal emulator's settings. Note that *this limitation only affects international/Unicode characters*. ASCII characters (a-z, A-Z, 0-9, punctuation) are typed directly and are unaffected.

**Non-ASCII characters come out wrong**: Increase `non-ascii-initial-delay` (and `non-ascii-default-delay`) to give the Wayland compositor more time to process the clipboard update before the paste keystroke arrives.

**Clipboard content is lost after dictation**: This should not happen — rhisper saves and restores your clipboard around any non-ASCII paste operations. If you see this, please open an issue.

---

## Development

Run the test suite:

```sh
cargo test
```

This runs unit tests (keymap tables for every supported layout, the IPC protocol, ASCII/Unicode paste-chunking, silence-detection parsing, config parsing) plus integration tests for the transcription provider against a local mock HTTP server (`tests/provider_test.rs`).

Every push and pull request against `main` runs `cargo build`, `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` on both Linux and macOS runners in CI (see `.github/workflows/ci.yml`). Tagged releases (`v*`) additionally build and publish `.deb`/`.rpm`/AUR-source/Homebrew-formula packages via `.github/workflows/release.yml`.

---

<p align="center">
  <em>Low complexity dictation for Linux</em>
</p>
