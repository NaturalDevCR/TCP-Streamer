# Linux User Guide

## Installing & Updating

Unlike Windows, where an installer wizard guides you through updates, Linux package management is more powerful but behaves differently.

### Updating to a new version

When you download a new `.deb` file, **do not double-click it** to install if you already have an older version. The basic GUI installer often fails to detect that it's an upgrade and may give cryptic "conflict" errors.

Instead, use the following standard command in your terminal. This command will correctly detect the installed version and upgrade it in-place, keeping your settings.

1.  Open your terminal.
2.  Navigate to where you downloaded the file (usually `~/Downloads`).
3.  Run the install command:

```bash
sudo apt install ./tcp-streamer_*.deb
```

_Note: The `./` is important — it tells `apt` to install the file from the current directory rather than searching online repositories._

### Troubleshooting

If you see an error saying "Conflicts with existing package", run:

```bash
sudo apt remove tcp-streamer
sudo apt install ./tcp-streamer_*.deb
```

## Audio Capture on Linux

TCP Streamer captures audio from ALSA/PulseAudio/PipeWire devices. To stream system audio output:

1. Find your monitor source:

   ```bash
   pactl list sources | grep -i monitor
   ```

2. In TCP Streamer, select the monitor source (e.g., `alsa_output.pci-...*.monitor`) as the Input Device.

For advanced setup (virtual devices, routing), use `pavucontrol` or PipeWire's `qpwgraph`.

## Native UDP & mDNS

- mDNS discovery requires the `avahi-daemon` (or compatible) service to be running. Most desktop Linux distributions enable this by default.
- If discovered sources don't appear, verify multicast is allowed on your network interface and no firewall blocks port 5353 (mDNS).

## Firewall

Ensure the TCP/UDP ports you use in TCP Streamer are open. For `ufw`:

```bash
sudo ufw allow 1704/tcp   # TCP server/client
sudo ufw allow 1704/udp   # Native UDP
```

## Real-time Thread Priority

The `.deb` package automatically runs a post-install script that sets `CAP_SYS_NICE` on the binary, allowing real-time thread priority for the audio capture thread. This reduces the risk of audio glitches under CPU load.
