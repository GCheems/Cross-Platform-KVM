# ==============================================================================
# 破壁者 (Cross-Platform KVM) - Windows 一键打包脚本
# ==============================================================================
# 用法: .\scripts\build_windows.ps1 [-Debug]
#
# 此脚本会自动完成以下步骤：
# 1. 检查并安装必要的依赖
# 2. 构建 Release 版本
# 3. 创建便携版 ZIP
# 4. 生成 MSI 安装程序 (如果安装了 WiX)
# ==============================================================================

param(
    [switch]$Debug,
    [switch]$SkipMsi
)

# 颜色函数
function Write-Header {
    param([string]$Message)
    Write-Host ""
    Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Cyan
    Write-Host "  $Message" -ForegroundColor Cyan
    Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Cyan
}

function Write-Step {
    param([string]$Message)
    Write-Host "▶ $Message" -ForegroundColor Green
}

function Write-Warning2 {
    param([string]$Message)
    Write-Host "⚠ $Message" -ForegroundColor Yellow
}

function Write-Error2 {
    param([string]$Message)
    Write-Host "✖ $Message" -ForegroundColor Red
}

function Write-Success {
    param([string]$Message)
    Write-Host "✔ $Message" -ForegroundColor Green
}

# 项目配置
$AppName = "Cross-Platform KVM"
$AppIdentifier = "com.kvm.cross-platform"
$Version = "0.1.0"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Split-Path -Parent $ScriptDir

# 构建类型
if ($Debug) {
    $BuildType = "debug"
} else {
    $BuildType = "release"
}

# 设置错误处理
$ErrorActionPreference = "Stop"

# ==============================================================================
# 步骤1: 检查依赖
# ==============================================================================
Write-Header "步骤 1/4: 检查依赖"

# 检查 Rust
Write-Step "检查 Rust..."
try {
    $rustVersion = rustc --version 2>$null
    if ($LASTEXITCODE -ne 0) { throw }
    Write-Success "Rust 版本: $rustVersion"
} catch {
    Write-Error2 "未找到 Rust。请从 https://rustup.rs/ 安装 Rust"
    Write-Host "运行: winget install Rustlang.Rust.GNU"
    exit 1
}

# 检查 Cargo
Write-Step "检查 Cargo..."
try {
    $cargoVersion = cargo --version 2>$null
    if ($LASTEXITCODE -ne 0) { throw }
    Write-Success "Cargo 已安装"
} catch {
    Write-Error2 "未找到 Cargo，请重新安装 Rust"
    exit 1
}

# 检查 Visual Studio Build Tools
Write-Step "检查 Visual Studio Build Tools..."
$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vsWhere) {
    $vsInstall = & $vsWhere -latest -property installationPath
    if ($vsInstall) {
        Write-Success "Visual Studio 已安装: $vsInstall"
    } else {
        Write-Warning2 "未找到 Visual Studio，编译可能失败"
    }
} else {
    Write-Warning2 "未找到 Visual Studio Installer"
}

# 检查 WiX Toolset
Write-Step "检查 WiX Toolset..."
$wixInstalled = $false
try {
    $candleVersion = candle -? 2>$null
    if ($LASTEXITCODE -eq 0 -or $candleVersion) {
        Write-Success "WiX Toolset 已安装"
        $wixInstalled = $true
    }
} catch {
    Write-Warning2 "WiX Toolset 未安装，将跳过 MSI 创建"
    Write-Host "  安装 WiX: winget install WiXToolset.WiXToolset"
}

# ==============================================================================
# 步骤2: 构建项目
# ==============================================================================
Write-Header "步骤 2/4: 构建项目"

Set-Location $ProjectDir

Write-Step "清理旧构建..."
if (Test-Path "$ProjectDir\dist") {
    Remove-Item -Recurse -Force "$ProjectDir\dist"
}
New-Item -ItemType Directory -Path "$ProjectDir\dist" | Out-Null

Write-Step "构建 $BuildType 版本..."
if ($BuildType -eq "release") {
    cargo build --release
    $BuildDir = "$ProjectDir\target\release"
} else {
    cargo build
    $BuildDir = "$ProjectDir\target\debug"
}

if ($LASTEXITCODE -ne 0) {
    Write-Error2 "构建失败"
    exit 1
}

if (-not (Test-Path "$BuildDir\kvm-ui.exe")) {
    Write-Error2 "构建失败：未找到 kvm-ui.exe"
    exit 1
}

Write-Success "构建完成"

# ==============================================================================
# 步骤3: 创建便携版 ZIP
# ==============================================================================
Write-Header "步骤 3/4: 创建便携版 ZIP"

$PortableDir = "$ProjectDir\dist\$AppName-$Version-portable"
$ZipPath = "$ProjectDir\dist\$AppName-$Version-windows-x64-portable.zip"

Write-Step "创建便携版目录..."
New-Item -ItemType Directory -Path $PortableDir | Out-Null

Write-Step "复制文件..."
Copy-Item "$BuildDir\kvm-ui.exe" "$PortableDir\$AppName.exe"
Copy-Item "$BuildDir\kvm.exe" "$PortableDir\kvm-cli.exe" -ErrorAction SilentlyContinue

# 复制图标
if (Test-Path "$ProjectDir\icons\icon.ico") {
    Copy-Item "$ProjectDir\icons\icon.ico" "$PortableDir\"
}

# 创建启动说明
$ReadmeContent = @"
$AppName v$Version
================================

使用方法：
1. 双击 "$AppName.exe" 启动应用
2. 首次运行需要管理员权限
3. 确保防火墙允许 UDP 5353 端口

配置防火墙（以管理员身份运行 PowerShell）：
netsh advfirewall firewall add rule name="Cross-Platform KVM" dir=in action=allow program="%CD%\$AppName.exe" enable=yes
netsh advfirewall firewall add rule name="Cross-Platform KVM UDP" dir=in action=allow protocol=UDP localport=5353 enable=yes

更多信息请访问项目主页。
"@
$ReadmeContent | Out-File -FilePath "$PortableDir\README.txt" -Encoding UTF8

Write-Step "创建 ZIP 压缩包..."
Compress-Archive -Path "$PortableDir\*" -DestinationPath $ZipPath -Force

Write-Success "便携版创建完成: $ZipPath"

# ==============================================================================
# 步骤4: 创建 MSI 安装程序 (可选)
# ==============================================================================
Write-Header "步骤 4/4: 创建 MSI 安装程序"

if ($SkipMsi) {
    Write-Warning2 "跳过 MSI 创建 (使用了 -SkipMsi 参数)"
} elseif (-not $wixInstalled) {
    Write-Warning2 "跳过 MSI 创建 (WiX Toolset 未安装)"
    Write-Host "  提示: 安装 WiX 后重新运行此脚本可创建 MSI"
} else {
    Write-Step "生成 WiX 配置..."
    
    $WxsPath = "$ProjectDir\dist\installer.wxs"
    $MsiPath = "$ProjectDir\dist\$AppName-$Version-windows-x64.msi"
    
    # 生成 GUID
    $ProductGuid = [guid]::NewGuid().ToString().ToUpper()
    $ComponentGuid = [guid]::NewGuid().ToString().ToUpper()
    $UpgradeGuid = "A1B2C3D4-E5F6-7890-ABCD-EF1234567890"  # 固定的升级 GUID
    
    $WxsContent = @"
<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="$ProductGuid" 
             Name="$AppName" 
             Language="1033" 
             Version="$Version.0" 
             Manufacturer="KVM Team" 
             UpgradeCode="$UpgradeGuid">
        
        <Package InstallerVersion="200" 
                 Compressed="yes" 
                 InstallScope="perMachine"
                 Description="$AppName Installer"
                 Comments="Cross-platform keyboard and mouse sharing"/>
        
        <MajorUpgrade DowngradeErrorMessage="已安装更新的版本。"/>
        <MediaTemplate EmbedCab="yes"/>
        
        <Feature Id="ProductFeature" Title="$AppName" Level="1">
            <ComponentGroupRef Id="ProductComponents"/>
            <ComponentRef Id="ApplicationShortcut"/>
        </Feature>
        
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Directory Id="ProgramFilesFolder">
                <Directory Id="INSTALLDIR" Name="$AppName">
                    <Component Id="MainExecutable" Guid="$ComponentGuid">
                        <File Id="MainExe" 
                              Source="$BuildDir\kvm-ui.exe" 
                              Name="$AppName.exe"
                              KeyPath="yes"/>
                    </Component>
                </Directory>
            </Directory>
            <Directory Id="ProgramMenuFolder">
                <Directory Id="ApplicationProgramsFolder" Name="$AppName"/>
            </Directory>
        </Directory>
        
        <DirectoryRef Id="ApplicationProgramsFolder">
            <Component Id="ApplicationShortcut" Guid="*">
                <Shortcut Id="ApplicationStartMenuShortcut"
                          Name="$AppName"
                          Description="Cross-platform KVM system"
                          Target="[INSTALLDIR]$AppName.exe"
                          WorkingDirectory="INSTALLDIR"/>
                <RemoveFolder Id="CleanUpShortCut" Directory="ApplicationProgramsFolder" On="uninstall"/>
                <RegistryValue Root="HKCU" Key="Software\$AppName" Name="installed" Type="integer" Value="1" KeyPath="yes"/>
            </Component>
        </DirectoryRef>
        
        <ComponentGroup Id="ProductComponents" Directory="INSTALLDIR">
        </ComponentGroup>
        
        <!-- UI -->
        <UIRef Id="WixUI_Minimal"/>
        <WixVariable Id="WixUILicenseRtf" Value="$ProjectDir\LICENSE.rtf"/>
        
    </Product>
</Wix>
"@

    # 创建简单的 LICENSE.rtf
    $LicenseRtf = @"
{\rtf1\ansi\deff0
{\fonttbl{\f0 Arial;}}
\f0\fs20
MIT License\par
\par
Copyright (c) 2024 KVM Team\par
\par
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:\par
\par
The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.\par
\par
THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.\par
}
"@
    $LicenseRtf | Out-File -FilePath "$ProjectDir\LICENSE.rtf" -Encoding ASCII
    
    # 写入 WXS 文件
    $WxsContent | Out-File -FilePath $WxsPath -Encoding UTF8
    
    Write-Step "编译 MSI..."
    try {
        $wixObjPath = "$ProjectDir\dist\installer.wixobj"
        
        # 编译 WXS 到 WIXOBJ
        & candle.exe -nologo -out $wixObjPath $WxsPath
        
        if ($LASTEXITCODE -eq 0) {
            # 链接生成 MSI
            & light.exe -nologo -out $MsiPath $wixObjPath -ext WixUIExtension
            
            if ($LASTEXITCODE -eq 0 -and (Test-Path $MsiPath)) {
                Write-Success "MSI 创建完成: $MsiPath"
            } else {
                Write-Warning2 "MSI 链接失败"
            }
        } else {
            Write-Warning2 "WiX 编译失败"
        }
        
        # 清理临时文件
        Remove-Item "$ProjectDir\dist\*.wixobj" -ErrorAction SilentlyContinue
        Remove-Item "$ProjectDir\dist\*.wixpdb" -ErrorAction SilentlyContinue
        
    } catch {
        Write-Warning2 "MSI 创建过程出错: $_"
    }
}

# ==============================================================================
# 完成
# ==============================================================================
Write-Header "打包完成！"

Write-Host ""
Write-Host "输出文件:" -ForegroundColor Green
Write-Host "  📦 便携版: $ZipPath"

$zipSize = (Get-Item $ZipPath).Length / 1MB
Write-Host "  📊 ZIP 大小: $([math]::Round($zipSize, 2)) MB"

if (Test-Path "$ProjectDir\dist\*.msi") {
    $msiFile = Get-ChildItem "$ProjectDir\dist\*.msi" | Select-Object -First 1
    Write-Host "  💿 安装程序: $($msiFile.FullName)"
    $msiSize = $msiFile.Length / 1MB
    Write-Host "  📊 MSI 大小: $([math]::Round($msiSize, 2)) MB"
}

Write-Host ""
Write-Host "安装说明:" -ForegroundColor Yellow
Write-Host "  便携版: 解压 ZIP 后直接运行"
Write-Host "  MSI: 双击安装程序进行安装"
Write-Host "  首次运行需要管理员权限和防火墙配置"
Write-Host ""
Write-Success "一键打包完成！"
