# Loupe

A simple image viewer application written with GTK4 and Rust.

![Loupe Screenshot](https://gitlab.gnome.org/Incubator/loupe/uploads/863131c1292cb9f1b32fbef39f266bcf/image.png)

## Installing

The latest version from the main branch is available from the build artifacts.

- [Download Loupe Development Version](https://gitlab.gnome.org/api/v4/projects/13923/jobs/artifacts/main/raw/org.gnome.Loupe.Devel.flatpak?job=flatpak)

This version will not get any automated updates after installation.

## Building

### GNOME Builder

GNOME Builder is the environment used for developing this application. It can use Flatpak manifests to create a consistent building and running environment cross-distro. Thus, it is highly
recommended you use it.

1. Download [GNOME Builder](https://flathub.org/apps/details/org.gnome.Builder).
2. In Builder, click the "Clone Repository" button at the bottom, using `git@gitlab.gnome.org/Incubator/loupe.git`
or `https://gitlab.gnome.org/Incubator/loupe.git` as the URL.
3. Click the build button at the top once the project is loaded.


## Installation

Depending on how you want it installed instructions can differ. If you
used GNOME Builder to build it, clicking the bar at the top window will 
open a submenu with "Export Bundle". This will create a flatpak bundle, 
which can be installed on any system that supports flatpak.

**In order for the Loupe flatpak to be able to read the directory images are installed in, you must install a bundle.**
Once you have a bundle installed, development builds will work properly.

## Contributing

- [Code Documentation](https://incubator.pages.gitlab.gnome.org/loupe/doc/loupe/)

### Conduct

Loupe operates under the GNOME Code Of Conduct. See the full
text of the Code Of Conduct [here](CODE_OF_CONDUCT.md).
