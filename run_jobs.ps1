cargo nextest run

$ProgressPreference = 'SilentlyContinue'
Invoke-WebRequest -Uri https://uploader.codecov.io/latest/windows/codecov.exe -Outfile codecov.exe
cargo llvm-cov nextest --locked --all-features --lcov --output-path lcov.info
.\codecov.exe -t ${CODECOV_TOKEN}
git rev-parse HEAD