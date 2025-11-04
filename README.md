# `de-switcher`

![Preview Image](./images/Preview.jpg)

A Rust TUI for quickly switching between desktop environments on EndeavourOS.

The TUI provides a simple interactive interface to:

1. Select a target DE/WM from a list of available profiles.
2. Choose the preferred package manager (`pacman`, `yay`, or `paru`).
3. Specify the output path for the generated script.

## Script generation

The TUI itself does **not** perform any system modifications. Instead, it
generates a complete, self-contained **Bash script** based on your selections.

This script is designed to handle the entire switching process, including:

* Removing packages associated with the current DE/WM profile.
* Installing the required packages for the target DE/WM.
* Disabling the old display manager (DM) and enabling the new, appropriate
Display Manager (`gdm`, `sddm`, `lightdm`, etc.).
* Prompting for reboot.

## Supported Desktop Environments

For desktop environments not listed in `eos-packagelist --list`, you would have to
to manually uninstall the old Desktop environment before running the script.

### Using the script

**NOTE:** The generated script must be executed outside of your current
graphical environment to avoid dependency conflicts and display issues.

1. **Run the TUI:** Execute the compiled Rust binary to generate the script.
2. **Review the Script:** Always review the generated script's contents before
execution to ensure no unwanted packages are scheduled for removal.
3. **Execute in TTY Mode:** Log out of your graphical session and switch to a
plain text terminal (TTY) using **`Ctrl+Alt+F4`** or a similar key combination.
4. **Run the Script:** Execute the script from the TTY using `./your_generated_script.sh`.

### License

This project is licensed under the **GNU General Public License v3.0**.

See [LICENSE](./LICENSE).
