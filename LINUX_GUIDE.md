# Linux User Guide

## Installing & Updating

Updates are installed manually from release assets; there is no configured in-app automatic updater. Download the package for your architecture.

### Updating to a new version

Use `apt` to install or upgrade a downloaded `.deb`, resolving package dependencies. Some desktop package installers also support upgrades.

Instead, use the following standard command in your terminal. This command will correctly detect the installed version and upgrade it in-place, keeping your settings.

Replace `VERSION_ARCH` with the exact downloaded filename; do not install every version matched by a wildcard.

1.  Open your terminal.
2.  Navigate to where you downloaded the file (usually `~/Downloads`).
3.  Run the install command:

```bash
sudo apt install ./tcp-streamer_VERSION_ARCH.deb
```

_Note: The `./` is important — it tells `apt` to install the file from the current directory rather than searching online repositories._

### Troubleshooting

If you see an error saying "Conflicts with existing package", run:

```bash
sudo apt remove tcp-streamer
sudo apt install ./tcp-streamer_VERSION_ARCH.deb
```

## Audio Capture on Linux

TCP Streamer uses CPAL's Linux ALSA backend. PulseAudio/PipeWire routing is available when exposed through your ALSA configuration; a monitor source listed by `pactl` is not guaranteed to appear directly in CPAL. To stream system audio output:

1. Find your monitor source:

   ```bash
   pactl list sources | grep -i monitor
   ```

2. If the monitor is exposed in TCP Streamer, select it as the Input Device. Otherwise route system audio into a capture device exposed through ALSA, or use `pavucontrol` to redirect the app's capture stream to the monitor when your sound server exposes it.

For advanced setup (virtual devices, routing), use `pavucontrol` or PipeWire's `qpwgraph`.

## Native UDP & mDNS

- Native UDP supports dual-stack listening where the host supports it, with IPv4 fallback; enter IPv6 source endpoints as `[address]:port`. The application advertises and discovers through its embedded `mdns-sd` implementation; installing Avahi is not required by the application.
- If discovered sources don't appear, verify multicast is allowed on your network interface and no firewall blocks port 5353 (mDNS).

## Firewall

Ensure the TCP/UDP ports you use in TCP Streamer are open. For `ufw`:

```bash
sudo ufw allow 1704/tcp   # Incoming TCP server connections
sudo ufw allow 1704/udp   # Native UDP
```

## Real-time Thread Priority

The `.deb` package automatically runs a post-install script that sets `CAP_SYS_NICE` on the binary, allowing real-time thread priority for the audio capture thread. This reduces the risk of audio glitches under CPU load.
