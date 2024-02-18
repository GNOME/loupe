# Loupe Help

The help pages are currently written in [ducktype](http://projectmallard.org/ducktype/1.0/index.html). The files are stored in `help/C/duck` and the corresponding `.page`-files can be generated via `make -C help/C`. Afterwards, you can preview the generated help pages via `yelp help/C/index.page`. The generated `.page`-files have to be committed to the repository as well. The `ducktype` program required for running `make` is probably packaged in you distro and is also [availabe on GitHub](https://github.com/projectmallard/mallard-ducktype).
