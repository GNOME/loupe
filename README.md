# GNOME Image Viewer

A simple image viewer application written with GTK4 and Rust.

## MVP TODO

* [ ] Hover timeout for buttons and fullscreen headerbar
* [ ] Pan/zoom
* [x] Back/forward navigation

## Building

### GNOME Builder

GNOME Builder is the environment used for developing this application. It can use Flatpak manifests to create a consistent building and running environment cross-distro. Thus, it is highly
recommended you use it.

1. Download [GNOME Builder](https://flathub.org/apps/details/org.gnome.Builder).
2. In Builder, click the "Clone Repository" button at the bottom, using `git@gitlab.gnome.org/BrainBlasted/gnome-image-viewer.git`
or `https://gitlab.gnome.org/BrainBlasted/gnome-image-viewer.git` as the URL.
3. Click the build button at the top once the project is loaded.


## Installation

Depending on how you want it installed instructions can differ. If you
used GNOME Builder to build it, clicking the bar at the top window will 
open a submenu with "Export Bundle". This will create a flatpak bundle, 
which can be installed on any system that supports flatpak.

## Conduct

GNOME Image Viewer operates under the GNOME Code Of Conduct. See the full
text of the Code Of Conduct [here](CODE_OF_CONDUCT.md).
