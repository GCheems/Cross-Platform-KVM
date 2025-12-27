@echo off
echo ========================================
echo 安装 Visual Studio C++ 构建工具
echo ========================================
echo.
echo 这将安装 MSVC C++ 编译器和相关工具
echo 需要管理员权限，请稍候...
echo.

"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vs_installer.exe" modify ^
    --installPath "%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools" ^
    --add Microsoft.VisualStudio.Workload.VCTools ^
    --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 ^
    --add Microsoft.VisualStudio.Component.Windows11SDK.22000 ^
    --includeRecommended ^
    --passive

echo.
echo ========================================
echo 安装完成！
echo ========================================
echo.
echo 请关闭所有命令行窗口，然后重新打开
echo 再运行: .\scripts\build_windows.ps1
echo.
pause
