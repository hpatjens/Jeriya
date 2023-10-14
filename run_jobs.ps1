# Run cargo nextest
$process_nextest = Start-Process -FilePath "cargo" -ArgumentList "nextest run" -NoNewWindow -Wait -PassThru
if ($process_nextest.ExitCode -ne 0) {
    Write-Host "cargo nextest run returned a non-zero exit code: $($process.ExitCode)" -ForegroundColor Red
    exit
} else {
    Write-Host "cargo nextest run completed successfully." -ForegroundColor Green
}

# Run cargo test --doc
$process_doctest = Start-Process -FilePath "cargo" -ArgumentList "test --doc" -NoNewWindow -Wait -PassThru
if ($process_doctest.ExitCode -ne 0) {
    Write-Host "cargo test --doc returned a non-zero exit code: $($process.ExitCode)" -ForegroundColor Red
    exit
} else {
    Write-Host "cargo test --doc completed successfully." -ForegroundColor Green
}

# Run Codecov
$ProgressPreference = 'SilentlyContinue'
Invoke-WebRequest -Uri https://uploader.codecov.io/latest/windows/codecov.exe -Outfile codecov.exe

cargo llvm-cov nextest --locked --lcov --output-path lcov.info

.\codecov.exe -t ${CODECOV_TOKEN}
if ($?) {
    Write-Host "Codecov Successful" -ForegroundColor Green
} else {
    Write-Host "Codecov Failed" -ForegroundColor Red
}

Write-Host "Git Commit: $(git rev-parse HEAD)"  -ForegroundColor Cyan
