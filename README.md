# Image Viewer (Loupe)

<a href='https://flathub.org/apps/org.gnome.Loupe'><img width='240' alt='Download on Flathub' src='https://flathub.org/api/badge?svg&locale=en'/></a>

GNOME's default image viewer providing

- Fast GPU-accelerated image rendering with tiled rendering for SVGs
- Extendable and sandboxed image decoding
- Support for more than 15 image formats by default
- Extensive support for touchpad and touchscreen gestures
- Accessible presentation of the most important metadata
- Sleek but powerful interface developed in conjunction with GNOME Human Interface Guidelines

![Image Viewer Screenshot](https://static.gnome.org/appdata/gnome-45/loupe/loupe-main.png)

## Supported Image Formats

Image Viewer uses [glycin](https://gitlab.gnome.org/sophie-h/glycin) for loading images. You can check [glycin's README](https://gitlab.gnome.org/sophie-h/glycin#supported-image-formats) for more details about the formats supported by the default loaders. However, glycin supports adding loaders for additional formats. Therefore, the supported formats on your system may vary and might be changed by installing or removing glycin loaders.

## Contributing

Contributions of all kinds are very welcome. Check the

- [Welcome to GNOME: Loupe](https://welcome.gnome.org/app/Loupe/)

pages for more information. If you are manually building Loupe on your system with Builder, make sure that you also have installed the [nightly version](https://welcome.gnome.org/app/Loupe/#installing-a-nightly-build) for all features to work. Otherwise, the development version will not have the required Flatpak permissions.

Documentation of Loupe's code is available online:

- [Loupe Code Documentation](https://gnome.pages.gitlab.gnome.org/loupe/doc/loupe/)

### Issue Tracker

Issues labeled as ~"1. Feature" or ~"1. Enhancement" are accepted for implementation. Feature requests that still need a decission are labeld as ~"2. RFC".

### Code of Conduct

When interacting with the project, the [GNOME Code Of Conduct](https://conduct.gnome.org/) applies.
