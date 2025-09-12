project(
  'loupe',
  'rust',
  version: '49.0',
  meson_version: '>= 0.59.0',
  default_options: [
    'warning_level=2',
  ],
)

i18n = import('i18n')
gnome = import('gnome')

dependency('gtk4', version: '>=4.16.0')
dependency('libadwaita-1', version: '>=1.7.0')
dependency('gweather4', version: '>=4.0.0')
# Required by glycin crate
dependency('lcms2', version: '>=2.12.0')
dependency('libseccomp', version: '>= 2.5.0')

# Optional dependencies
gtk4_x11 = dependency('gtk4-x11', required: get_option('x11'))

cargo = find_program('cargo', required: true)
glib_compile_schemas = find_program('glib-compile-schemas', required: true)

version_suffix = ''

if get_option('profile') != 'release'
  vcs_tag = run_command('git', 'rev-parse', '--short', 'HEAD', check: false).stdout().strip()
  if vcs_tag == ''
    version_suffix = '-devel'
  else
    version_suffix = '-@0@'.format(vcs_tag)
  endif
endif

prefix = get_option('prefix')

app_id = 'org.gnome.Loupe@0@'.format(get_option('app_id_suffix'))
profile = get_option('profile')
version = meson.project_version() + version_suffix
localedir = prefix / get_option('localedir')
bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')
pkgdatadir = datadir / meson.project_name()

subdir('data')
subdir('help')
subdir('src')
subdir('po')

gnome.post_install(
  gtk_update_icon_cache: true,
  update_desktop_database: true,
  glib_compile_schemas: true,
)
