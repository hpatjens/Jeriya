cargo nextest run

$ProgressPreference = 'SilentlyContinue'
Invoke-WebRequest -Uri https://uploader.codecov.io/latest/windows/codecov.exe -Outfile codecov.exe

cargo llvm-cov nextest --locked --all-features --lcov --output-path lcov.info

.\codecov.exe -t ${CODECOV_TOKEN}
if ($?) {
    Write-Host "Codecov Successful" -ForegroundColor Green
} else {
    Write-Host "Codecov Failed" -ForegroundColor Red
}

Write-Host "Git Commit: $(git rev-parse HEAD)"  -ForegroundColor Cyan
