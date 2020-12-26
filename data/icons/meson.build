scalable_dir = join_paths('hicolor', 'scalable', 'apps')

install_data(
  join_paths(scalable_dir, 'org.gnome.ImageViewer.svg'),
  install_dir: join_paths(iv_datadir, 'icons', scalable_dir),
)

install_data(
  join_paths(scalable_dir, 'org.gnome.ImageViewer.Devel.svg'),
  install_dir: join_paths(iv_datadir, 'icons', scalable_dir),
)

symbolic_dir = join_paths('hicolor', 'symbolic', 'apps')

install_data(
  join_paths(symbolic_dir, 'org.gnome.ImageViewer-symbolic.svg'),
  install_dir: join_paths(iv_datadir, 'icons', symbolic_dir),
  rename: '@0@-symbolic.svg'.format(app_id)
)
