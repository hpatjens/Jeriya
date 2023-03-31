git push

cargo nextext run

$ProgressPreference = 'SilentlyContinue'
Invoke-WebRequest -Uri https://uploader.codecov.io/latest/windows/codecov.exe -Outfile codecov.exe
cargo llvm-cov --locked --all-features --lcov --output-path lcov.info
.\codecov.exe -t ${CODECOV_TOKEN}