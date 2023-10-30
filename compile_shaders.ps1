$shader_path = "jeriya_backend_ash_base\test_data\"
$filesToProcess = Get-ChildItem -Path $shader_path -File | Where-Object { $_.Extension -ne ".spv" }

foreach ($file in $filesToProcess) {  
    $file = $file.FullName
    $fullCommand = "glslc `"${file}`" -o `"${file}.spv`""
    Write-Host $fullCommand -ForegroundColor Cyan
    Invoke-Expression -Command $fullCommand
}