scalable_dir = 'hicolor' / 'scalable' / 'apps'

install_data(
  scalable_dir / '@0@.svg'.format(app_id),
  install_dir: datadir / 'icons' / scalable_dir,
)

symbolic_dir = 'hicolor' / 'symbolic' / 'apps'

install_data(
  symbolic_dir / 'org.gnome.Loupe-symbolic.svg',
  install_dir: datadir / 'icons' / symbolic_dir,
  rename: '@0@-symbolic.svg'.format(app_id),
)
