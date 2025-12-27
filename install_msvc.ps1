# 以管理员身份运行此脚本来安装 MSVC C++ 工具链
# 右键点击此文件，选择"以管理员身份运行"

Write-Host "正在安装 Visual Studio Build Tools 和 C++ 工作负载..." -ForegroundColor Green
Write-Host "这可能需要几分钟时间，请耐心等待..." -ForegroundColor Yellow
Write-Host ""

$installerPath = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vs_installer.exe"

if (Test-Path $installerPath) {
    & $installerPath modify `
        --installPath "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools" `
        --add Microsoft.VisualStudio.Workload.VCTools `
        --includeRecommended `
        --passive
    
    Write-Host ""
    Write-Host "安装完成！" -ForegroundColor Green
    Write-Host "请关闭所有 PowerShell 窗口，然后重新打开，再运行打包脚本。" -ForegroundColor Yellow
} else {
    Write-Host "未找到 Visual Studio Installer" -ForegroundColor Red
    Write-Host "请手动运行: winget install Microsoft.VisualStudio.2022.BuildTools" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "按任意键退出..."
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
