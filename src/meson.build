configure_file(input: 'config.rs.in', output: 'config.rs', configuration: conf)

build_root = meson.project_build_root()
source_root = meson.project_source_root()

# Copy the config.rs output to the source directory.
run_command(
  'cp',
  build_root / 'src' / 'config.rs',
  source_root / 'src' / 'config.rs',
  check: true,
)

cargo_options = ['--manifest-path', source_root / 'Cargo.toml']
cargo_options += ['--target-dir', build_root / 'src']

if get_option('profile') == 'development'
  rust_target = 'debug'
  message('Building in debug mode')
else
  cargo_options += ['--release']
  rust_target = 'release'
  message('Building in release mode')
endif

if gtk4_x11.found()
  cargo_options += ['--features', 'x11']
endif

cargo_env = {'CARGO_HOME': build_root / 'cargo-home'}

cargo_build = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: 'src',
  console: true,
  env: cargo_env,
  command: [
    cargo,
    'build',
    cargo_options,
  ],
)

custom_target(
  'cp-binary',
  depends: cargo_build,
  build_by_default: true,
  build_always_stale: true,
  install: true,
  install_dir: get_option('bindir'),
  output: meson.project_name(),
  command: [
    'cp',
    'src' / rust_target / meson.project_name(),
    '@OUTPUT@',
  ],
)

custom_target(
  'cargo-doc',
  build_by_default: false,
  build_always_stale: true,
  output: 'doc',
  console: true,
  command: [
    cargo,
    'doc',
    cargo_options + ['--no-deps', '--document-private-items'],
  ],
)

test(
  'cargo-test',
  cargo,
  args: [
    'test',
    cargo_options,
    '--',
    '--test-threads=1',
  ],
  is_parallel: false,
  timeout: 1800,
)