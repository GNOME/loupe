mime_types = []

# image-rs
mime_types += [
  'image/jpeg',
  'image/png',
  'image/gif',
  'image/webp',
  'image/tiff',
  # TGA
  'image/x-tga',
  # DDS
  'image/vnd-ms.dds',
  'image/x-dds',
  'image/bmp',
  # .ico
  'image/vnd.microsoft.icon',
  # HDR
  'image/vnd.radiance',
  # OpenEXR
  'image/x-exr',
  # PNM
  'image/x-portable-bitmap',
  'image/x-portable-graymap',
  'image/x-portable-pixmap',
  'image/x-portable-anymap',
  # Qoi's MIME type is being worked on.
  # See: https://github.com/phoboslab/qoi/issues/167
  'image/x-qoi',
]

# SVG
mime_types += [
  'image/svg+xml',
  'image/svg+xml-compressed',
]

# HEIF
mime_types += [
  'image/avif',
  'image/heic',
]

# JXL
mime_types += [
  'image/jxl',
]

conf.set('MIME_TYPES', ';'.join(mime_types) + ';')

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

# GSchema
gschema_conf = configuration_data()
gschema_conf.set('app-id', app_id)
gschema_conf.set('gettext-package', meson.project_name())
configure_file(
  input: '@0@.gschema.xml.in'.format(base_id),
  output: '@0@.gschema.xml'.format(app_id),
  configuration: gschema_conf,
  install: true,
  install_dir: iv_datadir / 'glib-2.0' / 'schemas'
)

# Validate GSchema
test(
  'validate-gschema', glib_compile_schemas,
  args: [
    '--strict', '--dry-run', meson.current_source_dir()
  ]
)

# Post install

gnome.post_install()
