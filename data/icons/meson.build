scalable_dir = 'hicolor' / 'scalable' / 'apps'

install_data(
  scalable_dir / 'org.gnome.Loupe.svg',
  install_dir: iv_datadir / 'icons' / scalable_dir,
)

install_data(
  scalable_dir / 'org.gnome.Loupe.Devel.svg',
  install_dir: iv_datadir / 'icons' / scalable_dir,
)

symbolic_dir = 'hicolor' / 'symbolic' / 'apps'

install_data(
  symbolic_dir / 'org.gnome.Loupe-symbolic.svg',
  install_dir: iv_datadir / 'icons' / symbolic_dir,
  rename: '@0@-symbolic.svg'.format(app_id)
)
