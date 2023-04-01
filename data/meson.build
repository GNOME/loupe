desktop_file = i18n.merge_file(
  input: configure_file(
    input: 'org.gnome.Loupe.desktop.in.in',
    output: 'org.gnome.Loupe.desktop.in',
    configuration: conf,
  ),
  output: '@0@.desktop'.format(app_id),
  type: 'desktop',
  po_dir: '../po',
  install: true,
  install_dir: iv_datadir / 'applications'
)

configure_file(
    input: 'org.gnome.Loupe.service.in',
    output: '@0@.service'.format(app_id),
    configuration: conf,
    install: true,
    install_dir: iv_datadir / 'dbus-1' / 'services'
)

desktop_utils = find_program('desktop-file-validate', required: false)
if desktop_utils.found()
  test('Validate desktop file', desktop_utils,
    args: [desktop_file]
  )
endif

appstream_file = i18n.merge_file(
  input: configure_file(
    input: 'org.gnome.Loupe.metainfo.xml.in.in',
    output: 'org.gnome.Loupe.metainfo.xml.in',
    configuration: conf,
  ),
  output: '@0@.metainfo.xml'.format(app_id),
  po_dir: '../po',
  install: true,
  install_dir: iv_datadir / 'metainfo'
)

subdir('icons')

# Post install

gnome.post_install()
