# 构建脚本

本目录包含用于构建和打包 Cross-Platform KVM 应用的脚本。

## 可用脚本

### `build_windows.ps1`
Windows 平台一键打包脚本，自动创建便携版 ZIP 和 MSI 安装程序。

**用法：**
```powershell
# 默认 Release 构建
.\scripts\build_windows.ps1

# Debug 构建
.\scripts\build_windows.ps1 -Debug

# 跳过 MSI 生成
.\scripts\build_windows.ps1 -SkipMsi
```

**功能：**
- ✅ 自动检查 Rust、Cargo、Visual Studio 依赖
- ✅ 构建 Release 或 Debug 版本
- ✅ 创建便携版 ZIP（包含可执行文件和使用说明）
- ✅ 可选生成 MSI 安装程序（检测到 WiX Toolset 时）
- ✅ 彩色输出和进度提示

**依赖：**
- PowerShell 5.1+
- Rust 工具链（MSVC 或 GNU）
- Visual Studio Build Tools（MSVC 工具链）或 MinGW-w64（GNU 工具链）
- WiX Toolset 3.11+（可选，用于生成 MSI）

**输出：** `dist/` 目录
- `Cross-Platform KVM-0.1.0-windows-x64-portable.zip` - 便携版
- `Cross-Platform KVM-0.1.0-windows-x64.msi` - 安装程序（可选）

---

### `build_macos.sh`
macOS 平台一键打包脚本，自动创建 .app 应用包和 .dmg 安装镜像。

**用法：**
```bash
# 默认 Release 构建
./scripts/build_macos.sh

# Debug 构建
./scripts/build_macos.sh --debug
```

**功能：**
- ✅ 自动检查 Rust、Cargo、Xcode 依赖
- ✅ 构建 Release 或 Debug 版本
- ✅ 创建 .app 应用包（包含 Info.plist 和图标）
- ✅ 生成 .dmg 安装镜像
- ✅ 彩色输出和进度提示

**依赖：**
- Bash 4.0+
- Rust 工具链
- Xcode Command Line Tools
- create-dmg（可选，用于创建更美观的 DMG）

**输出：** `dist/` 目录
- `Cross-Platform KVM.app` - 应用包
- `Cross-Platform KVM_0.1.0_aarch64.dmg` - 安装镜像（ARM）
- `Cross-Platform KVM_0.1.0_x64.dmg` - 安装镜像（Intel）

---

## 快速参考

| 任务 | 命令 |
|------|---------|
| Windows 打包 | `.\scripts\build_windows.ps1` |
| macOS 打包 | `./scripts/build_macos.sh` |
| Windows Debug | `.\scripts\build_windows.ps1 -Debug` |
| macOS Debug | `./scripts/build_macos.sh --debug` |

## 构建输出位置

| 平台 | 输出目录 | 文件 |
|----------|-------------|---------------|
| Windows | `dist/` | `*.zip`, `*.msi` |
| macOS | `dist/` | `*.app`, `*.dmg` |

## 故障排除

### "Permission denied" 运行脚本时

**解决方案：**
```bash
chmod +x scripts/*.sh
```

### Windows "Execution policy" 错误

**解决方案：**
```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

或使用 bypass 运行：
```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build_windows.ps1
```

### 构建失败 "WiX not found"

**解决方案：** 安装 WiX Toolset
```powershell
winget install WiXToolset.WiXToolset
```

### 构建失败 "Visual Studio Build Tools not found"

**解决方案：** 安装 Visual Studio Build Tools
```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
```

### macOS 构建失败 "Xcode Command Line Tools not found"

**解决方案：** 安装 Xcode Command Line Tools
```bash
xcode-select --install
```

### Rust 工具链问题（Windows）

如果使用 GNU 工具链但缺少 MinGW-w64：

**推荐方案：** 切换到 MSVC 工具链
```powershell
# 1. 卸载当前 Rust
winget uninstall Rustlang.Rust.GNU

# 2. 安装 MSVC 版本
winget install Rustlang.Rust.MSVC

# 3. 安装 Visual Studio Build Tools
winget install Microsoft.VisualStudio.2022.BuildTools
```

**备选方案：** 安装 MinGW-w64
```powershell
winget install mingw-w64
```

## 贡献

添加新构建脚本时：

1. 使脚本可执行：`chmod +x script.sh`
2. 添加错误处理：bash 使用 `set -e`，PowerShell 使用 `$ErrorActionPreference = "Stop"`
3. 在此 README 中添加使用文档
4. 在干净环境中测试
