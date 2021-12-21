project('loupe', 'rust',
          version: '42.alpha',
    meson_version: '>= 0.50.0',
  default_options: [ 'warning_level=2',
                   ],
)

i18n = import('i18n')
gnome = import('gnome')

dependency('gtk4', version: '>=4.0.0')
dependency('libadwaita-1')

iv_prefix = get_option('prefix')
iv_bindir = join_paths(iv_prefix, get_option('bindir'))
iv_localedir = join_paths(iv_prefix, get_option('localedir'))
iv_datadir = join_paths(iv_prefix, get_option('datadir'))
iv_pkgdatadir = join_paths(iv_datadir, meson.project_name())

profile = ''
name_suffix = ''
version_suffix = ''

if get_option('profile') == 'development'
  profile = '.Devel'
  name_suffix = ' ☢'
  vcs_tag = run_command('git', 'rev-parse', '--short', 'HEAD').stdout().strip()
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
conf.set('bindir', iv_bindir)
conf.set('localedir', iv_localedir)
conf.set('pkgdatadir', iv_pkgdatadir)

subdir('data')
subdir('help')
subdir('src')
subdir('po')

# Pre-commit hook for rust formatting
if get_option('profile') == 'development'
    message('Setting up git pre-commit hook..')
    run_command('cp', '-f', 'hooks/pre-commit.hook', '.git/hooks/pre-commit')
endif

meson.add_install_script('build-aux/meson/postinstall.py')
