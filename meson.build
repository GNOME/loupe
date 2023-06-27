project('loupe', 'rust',
          version: '44.3',
    meson_version: '>= 0.59.0',
  default_options: [ 'warning_level=2',
                   ],
)

i18n = import('i18n')
gnome = import('gnome')

dependency('gtk4', version: '>=4.11.3')
dependency('libadwaita-1', version: '>=1.3.99')
dependency('gweather4', version: '>=4.0.0')
dependency('libheif', version: '>=1.14.2')
dependency('lcms2', version: '>=2.12.0')

cargo = find_program('cargo', required: true)

iv_prefix = get_option('prefix')
iv_bindir = iv_prefix / get_option('bindir')
iv_localedir = iv_prefix / get_option('localedir')
iv_datadir = iv_prefix / get_option('datadir')
iv_pkgdatadir = iv_datadir / meson.project_name()

profile = ''
version_suffix = ''

if get_option('profile') != 'release'
  profile = '.Devel'
  vcs_tag = run_command('git', 'rev-parse', '--short', 'HEAD', check: false).stdout().strip()
  if vcs_tag == ''
    version_suffix = '-devel'
  else
    version_suffix = '-@0@'.format(vcs_tag)
  endif
endif

app_id = 'org.gnome.Loupe@0@'.format(profile)
iv_version = meson.project_version() + version_suffix

cargo_sources = files(
  'Cargo.toml',
  'Cargo.lock',
)

conf = configuration_data()
conf.set('APP_ID', app_id)
conf.set('PROFILE', profile)
conf.set('VERSION', iv_version)
conf.set('BIN_NAME', meson.project_name())
conf.set('bindir', iv_bindir)
conf.set('localedir', iv_localedir)
conf.set('pkgdatadir', iv_pkgdatadir)

subdir('data')
subdir('src')
subdir('po')

# Pre-commit hook for rust formatting
if get_option('profile') == 'development'
    message('Setting up git pre-commit hook..')
    run_command('cp', '-f', 'hooks/pre-commit.hook', '.git/hooks/pre-commit', check: false)
endif

gnome.post_install(
  gtk_update_icon_cache: true,
  glib_compile_schemas: true,
  update_desktop_database: true,
)

meson.add_dist_script(
  'build-aux/meson/dist-vendor.sh',
  join_paths(meson.project_build_root(), 'meson-dist', meson.project_name() + '-' + meson.project_version()),
  meson.project_source_root()
)