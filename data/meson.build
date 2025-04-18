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
  # Both mime types exist in the wild
  'image/x-qoi',
  'image/qoi',
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

conf = configuration_data()
conf.set('APP_ID', app_id)
conf.set('BIN_NAME', meson.project_name())
conf.set('BINDIR', bindir)
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
  install_dir: datadir / 'applications',
)

configure_file(
  input: 'org.gnome.Loupe.service.in',
  output: '@0@.service'.format(app_id),
  configuration: conf,
  install: true,
  install_dir: datadir / 'dbus-1' / 'services',
)

desktop_utils = find_program('desktop-file-validate', required: false)
if desktop_utils.found()
  test('Validate desktop file', desktop_utils, args: [desktop_file])
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
  install_dir: datadir / 'metainfo',
)

# Validate Appdata
appstreamcli = find_program('appstreamcli', required: false)
if (appstreamcli.found())
  test(
    'Validate metainfo file',
    appstreamcli,
    args: ['validate', '--no-net', appstream_file],
    workdir: meson.current_build_dir(),
  )
endif

subdir('icons')

# GSchema
gschema_conf = configuration_data()
gschema_conf.set('app-id', app_id)
gschema_conf.set('gettext-package', meson.project_name())
configure_file(
  input: 'org.gnome.Loupe.gschema.xml.in',
  output: '@0@.gschema.xml'.format(app_id),
  configuration: gschema_conf,
  install: true,
  install_dir: datadir / 'glib-2.0' / 'schemas',
)

# Validate GSchema
test(
  'validate-gschema',
  glib_compile_schemas,
  args: ['--strict', '--dry-run', meson.current_source_dir()],
)
