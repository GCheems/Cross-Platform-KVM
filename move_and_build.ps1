# 将项目移动到英文路径并构建
# 这个脚本会自动处理中文路径问题

$currentPath = Get-Location
$projectName = "cross-platform-kvm"
$newPath = "C:\Users\$env:USERNAME\Desktop\$projectName"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  解决中文路径问题" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "当前路径: $currentPath" -ForegroundColor Yellow
Write-Host "目标路径: $newPath" -ForegroundColor Green
Write-Host ""

if (Test-Path $newPath) {
    Write-Host "⚠ 目标路径已存在" -ForegroundColor Yellow
    $response = Read-Host "是否覆盖? (y/n)"
    if ($response -ne 'y') {
        Write-Host "操作已取消" -ForegroundColor Red
        exit 1
    }
    Remove-Item -Recurse -Force $newPath
}

Write-Host "正在复制项目文件..." -ForegroundColor Green
Copy-Item -Path $currentPath -Destination $newPath -Recurse -Force

Write-Host "✔ 项目已复制到: $newPath" -ForegroundColor Green
Write-Host ""
Write-Host "现在将在新路径下构建项目..." -ForegroundColor Green
Write-Host ""

Set-Location $newPath

# 加载 MSVC 环境变量
$vsPath = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools"
$vcvarsPath = "$vsPath\VC\Auxiliary\Build\vcvars64.bat"
if (Test-Path $vcvarsPath) {
    cmd /c "`"$vcvarsPath`" >nul 2>&1 && set" | ForEach-Object {
        if ($_ -match "^(.*?)=(.*)$") {
            Set-Item -Force -Path "env:$($matches[1])" -Value $matches[2]
        }
    }
    Write-Host "✔ MSVC 环境变量已加载" -ForegroundColor Green
}

Write-Host ""
Write-Host "开始构建..." -ForegroundColor Green
Write-Host ""

.\scripts\build_windows.ps1

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  完成！" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "新项目路径: $newPath" -ForegroundColor Green
Write-Host "构建输出: $newPath\dist" -ForegroundColor Green
Write-Host ""
