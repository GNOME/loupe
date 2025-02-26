# Contributing

Contributions of all kinds are very welcome. Check the

- [Welcome to GNOME: Loupe](https://welcome.gnome.org/app/Loupe/)

pages for more information. If you are manually building Loupe on your system with Builder, make sure that you also have installed the [nightly version](https://welcome.gnome.org/app/Loupe/#installing-a-nightly-build) for all features to work. Otherwise, the development version will not have the required Flatpak permissions.

Documentation of Loupe's code is available online:

- [Loupe Code Documentation](https://gnome.pages.gitlab.gnome.org/loupe/doc/loupe/)

## Issue Tracker

Issues labeled as ~"1. Feature" or ~"1. Enhancement" are accepted for implementation. Feature requests that still need a decission are labeld as ~"2. RFC".

## Code of Conduct

When interacting with the project, the [GNOME Code Of Conduct](https://conduct.gnome.org/) applies.

## Help format

The help pages are currently written in [ducktype](http://projectmallard.org/ducktype/1.0/index.html). The files are stored in `help/C/duck` and the corresponding `.page`-files can be generated via `make -C help/C/`. Afterwards, you can preview the generated help pages via `yelp help/C/index.page`. The generated `.page`-files have to be committed to the repository as well. The `ducktype` program required for running `make` is probably packaged in you distro and is also [availabe on GitHub](https://github.com/projectmallard/mallard-ducktype).
