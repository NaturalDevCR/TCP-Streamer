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

_Note: The `./` is important—it tells `apt` to install the file from the current directory rather than searching online repositories._

### Troubleshooting

If you see an error saying "Conflicts with existing package", run:

```bash
sudo apt remove tcp-streamer
sudo apt install ./tcp-streamer_*.deb
```
