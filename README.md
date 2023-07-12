# Image Viewer (Loupe)

An image viewer application written with GTK 4, Libadwaita and Rust.

![Image Viewer Screenshot](https://gitlab.gnome.org/GNOME/Incubator/loupe/uploads/24c80abc88fccb5fc9f2f08de6a7a5ea/screenshot1.png)

## Installing

The latest version from the main branch is available from the build artifacts.

Download bundle for:

* [x86_64](https://gitlab.gnome.org/api/v4/projects/13923/jobs/artifacts/main/raw/org.gnome.Loupe.Devel.flatpak?job=flatpak) (Average desktop or laptop PC)
* [aarch64](https://gitlab.gnome.org/api/v4/projects/13923/jobs/artifacts/main/raw/org.gnome.Loupe.Devel.flatpak?job=flatpak@aarch64) (Average phone, tablet, or Apple Silicon devices)

Bundles will not be automatically updated after installation.

## Features

- Fast GPU accelerated image rendering with tiled rendering for SVGs
- Extendable and sandboxed (expect SVG) image decoding
- Support for more than 15 image formats by default
- Extensive support for touchpad and touchscreen gestures
- Accessible presentation of the most important metadata
- Sleek but powerful interface developed in conjunction with GNOME Human Interface Guidelines

## Supported Formats

Image Viewer uses [glycin](https://gitlab.gnome.org/sophie-h/glycin) for loading images. You can check [glycin's README](https://gitlab.gnome.org/sophie-h/glycin#supported-image-formats) for more details about the formats supported by the default loaders. However, glycin supports adding loaders for additional formats. Therefore, the supported formats on your system may vary and might be changed by installing or removing glycin loaders.

## Building

### GNOME Builder

GNOME Builder is the environment used for developing this application. It can use Flatpak manifests to create a consistent building and running environment cross-distro. Thus, it is highly
recommended you use it.

1. Download [GNOME Builder](https://flathub.org/apps/details/org.gnome.Builder).
2. In Builder, click the "Clone Repository" button at the bottom, using `git@ssh.gitlab.gnome.org:GNOME/Incubator/loupe.git`
or `https://gitlab.gnome.org/GNOME/Incubator/loupe.git` as the URL.
3. Click the build button at the top once the project is loaded.


## Installation

Depending on how you want it installed instructions can differ. If you
used GNOME Builder to build it, clicking the bar at the top window will 
open a submenu with "Export Bundle". This will create a flatpak bundle,
which can be installed on any system that supports flatpak.

**In order for the Image Viewer flatpak to be able to read the directory images are installed in, you must install a bundle.**
Once you have a bundle installed, development builds will work properly.

## Contributing

- [Code Documentation](https://gnome.pages.gitlab.gnome.org/Incubator/loupe/doc/loupe/)

### Conduct

Image Viewer operates under the GNOME Code Of Conduct. See the full
text of the Code Of Conduct [here](CODE_OF_CONDUCT.md).
