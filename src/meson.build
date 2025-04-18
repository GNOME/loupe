build_root = meson.project_build_root()
source_root = meson.project_source_root()

cargo_options = ['--manifest-path', source_root / 'Cargo.toml']
cargo_options += ['--target-dir', build_root / 'src']
cargo_options += ['--profile', profile]

if gtk4_x11.found()
  cargo_options += ['--features', 'x11']
endif

if profile == 'dev'
  rust_target = 'debug'
elif profile == 'release'
  rust_target = 'release'
endif

cargo_env = {
  'CARGO_HOME': build_root / 'cargo-home',
  'APP_ID': app_id,
  'VERSION': version,
  'LOCALEDIR': localedir,
}

# Build binary
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

# Copy binary
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

# Run tests
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
