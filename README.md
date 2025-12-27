# 破壁者 (Cross-Platform KVM)

一个跨平台的键盘、鼠标共享系统，支持在同一局域网内的 Windows 和 macOS 设备之间无缝切换控制。

> ⚠️ **开发状态**: 本项目目前处于早期开发阶段，核心框架已完成，部分功能仍在实现中。

## 📋 目录

- [功能特性](#功能特性)
- [系统要求](#系统要求)
- [快速开始](#快速开始)
- [开发指南](#开发指南)
- [项目结构](#项目结构)

---

## 🚀 功能特性

### 已实现
- ✅ **基础架构** - 基于 Rust + Tauri 的跨平台框架
- ✅ **设备发现** - UDP 多播自动发现局域网设备
- ✅ **安全通信** - TLS 1.3 加密连接
- ✅ **配置管理** - 设备布局和快捷键配置
- ✅ **系统托盘** - 菜单栏/系统托盘集成

### 计划中
- � **屏幕边缘切换** - 像多显示器一样自然地在设备间移动鼠标
- 🔲 **键盘自动路由** - 键盘输入自动发送到当前活动设备
- � **剪切板同步** - 在设备间无缝复制粘贴文本和图像
- � **输入捕获** - 完整的鼠标和键盘事件捕获

---

## 💻 系统要求

### macOS
- 操作系统：macOS 10.15 Catalina 或更高版本
- 内存：至少 4GB RAM
- 磁盘空间：至少 20MB
- 权限：辅助功能和输入监控权限

### Windows
- 操作系统：Windows 10 1809 或更高版本
- 内存：至少 4GB RAM
- 磁盘空间：至少 20MB
- 权限：管理员权限

---

## ⚡ 快速开始

### 方式一：使用预构建版本（推荐）

1. **下载安装包**
   ```bash
   # 使用一键打包脚本生成安装包
   # macOS:
   ./scripts/build_macos.sh
   
   # Windows:
   .\scripts\build_windows.ps1
   ```

2. **安装应用**
   - **macOS**: 打开生成的 `.dmg` 文件，将应用拖入 Applications 文件夹
   - **Windows**: 运行生成的便携版 ZIP 或 MSI 安装程序

3. **授予权限**
   - **macOS**: 系统偏好设置 → 安全性与隐私 → 隐私 → 辅助功能/输入监控
   - **Windows**: 以管理员身份运行，允许防火墙访问

### 方式二：从源码构建

```bash
# 克隆仓库
git clone <repository-url>
cd 破壁者

# 构建 Release 版本
cargo build --release

# 运行 GUI 版本
./target/release/kvm-ui

# 或运行命令行版本
./target/release/kvm
```

**输出位置**:
- macOS/Linux: `target/release/kvm-ui`
- Windows: `target\release\kvm-ui.exe`

---

## 👨‍💻 开发指南

### 前置要求

- **Rust 1.70+** - [安装 Rust](https://rustup.rs/)
- **平台特定工具**：
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`)
  - **Windows**: Visual Studio Build Tools 2019+ 和 WebView2

### 快速构建

```bash
# 开发构建
cargo build

# 发布构建（优化版本）
cargo build --release

# 运行测试
cargo test
```

### 📦 一键打包

项目提供开箱即用的打包脚本，自动处理所有构建和打包步骤。

#### macOS 打包

```bash
# 给脚本添加执行权限
chmod +x scripts/build_macos.sh

# 运行打包脚本
./scripts/build_macos.sh

# 输出位置:
# - dist/Cross-Platform KVM.app       (应用包)
# - dist/Cross-Platform KVM_0.1.0_aarch64.dmg  (安装镜像)
```

**脚本功能**:
- ✅ 自动检查 Rust、Cargo、Xcode 依赖
- ✅ 构建 Release 版本
- ✅ 创建 `.app` 应用包（包含 Info.plist 和图标）
- ✅ 生成 `.dmg` 安装镜像
- ✅ 彩色输出和进度提示

#### Windows 打包

```powershell
# 运行打包脚本
.\scripts\build_windows.ps1

# 输出位置:
# - dist\Cross-Platform KVM-0.1.0-portable.zip  (便携版)
# - dist\Cross-Platform KVM-0.1.0-windows-x64.msi  (安装程序，需 WiX)
```

**脚本功能**:
- ✅ 自动检查 Rust、Visual Studio 依赖
- ✅ 构建 Release 版本
- ✅ 创建便携版 ZIP（包含可执行文件和使用说明）
- ✅ 可选生成 MSI 安装程序（检测到 WiX Toolset 时）
- ✅ 彩色输出和进度提示

#### 打包选项

```bash
# macOS - 调试版本打包
./scripts/build_macos.sh --debug

# Windows - 跳过 MSI 生成
.\scripts\build_windows.ps1 -SkipMsi

# Windows - 调试版本打包
.\scripts\build_windows.ps1 -Debug
```

### 架构概览

#### 核心模块架构

系统采用模块化设计，各模块职责清晰：

**网络层**
- `network.rs` - TCP/TLS 连接管理
- `discovery.rs` - UDP 多播设备发现
- `protocol.rs` - 通信协议定义
- `security.rs` - TLS 加密和证书管理

**输入层**
- `input.rs` - 输入系统抽象接口
- `input/macos.rs` - macOS 平台输入实现
- `input/windows.rs` - Windows 平台输入实现
- `batching.rs` - 输入事件批处理优化

**控制层**
- `switch.rs` - 设备切换控制逻辑
- `state.rs` - 全局状态管理
- `device.rs` - 设备信息和状态
- `hotkey.rs` - 全局热键管理

**服务层**
- `clipboard.rs` - 剪切板监控和同步
- `config.rs` - 配置文件管理
- `permissions.rs` - 系统权限检查
- `recovery.rs` - 错误恢复机制

**界面层**
- `ui.rs` - UI 状态和逻辑
- `tauri_commands.rs` - Tauri 命令处理
- `tauri_main.rs` - GUI 应用入口

#### 技术栈

- **后端**: Rust 2021，异步运行时 Tokio
- **网络**: TLS 1.3 (rustls), UDP 多播
- **GUI**: Tauri 1.5
- **前端**: 原生 HTML/CSS/JavaScript
- **平台 API**: 
  - Windows: windows-rs, winapi
  - macOS: core-graphics, core-foundation

---

## 📁 项目结构

```
破壁者/
├── src/                      # Rust 后端源代码
│   ├── lib.rs               # 库入口点，模块导出
│   ├── main.rs              # CLI 命令行入口点
│   ├── tauri_main.rs        # Tauri GUI 入口点
│   │
│   ├── network.rs           # 网络连接管理（TCP/TLS）
│   ├── discovery.rs         # UDP 多播设备发现
│   ├── protocol.rs          # 通信协议定义
│   ├── security.rs          # TLS 加密和证书管理
│   │
│   ├── device.rs            # 设备信息和状态
│   ├── switch.rs            # 设备切换控制逻辑
│   ├── state.rs             # 全局状态管理
│   │
│   ├── input.rs             # 输入系统抽象层
│   ├── input/               # 平台特定输入实现
│   │   ├── macos.rs         # macOS 输入捕获/注入
│   │   └── windows.rs       # Windows 输入捕获/注入
│   │
│   ├── clipboard.rs         # 剪切板监控和同步
│   ├── hotkey.rs            # 全局热键管理
│   ├── batching.rs          # 输入事件批处理
│   │
│   ├── config.rs            # 配置文件管理
│   ├── permissions.rs       # 系统权限检查
│   ├── recovery.rs          # 错误恢复机制
│   ├── error.rs             # 错误类型定义
│   │
│   ├── ui.rs                # UI 状态和逻辑
│   └── tauri_commands.rs    # Tauri 命令处理
│
├── ui/                       # Tauri 前端界面
│   ├── index.html           # 主页面
│   ├── app.js               # 主应用逻辑
│   ├── styles.css           # 全局样式
│   │
│   ├── layout.html          # 设备布局配置页面
│   ├── layout.js            # 布局配置逻辑
│   ├── layout.css           # 布局页面样式
│   │
│   ├── hotkey.html          # 热键配置页面
│   ├── hotkey.js            # 热键配置逻辑
│   ├── hotkey.css           # 热键页面样式
│   │
│   ├── onboarding.html      # 新手引导页面
│   ├── onboarding.js        # 引导逻辑
│   ├── onboarding.css       # 引导页面样式
│   │
│   ├── update.html          # 更新检查页面
│   ├── update.js            # 更新逻辑
│   ├── update.css           # 更新页面样式
│   │
│   ├── notifications.js     # 通知系统
│   ├── notifications.css    # 通知样式
│   │
│   ├── permissions-check.html  # 权限检查页面
│   ├── test-notifications.html # 通知测试页面
│   │
│   └── dist/                # 前端构建输出
│       ├── index.html
│       ├── app.js
│       └── styles.css
│
├── examples/                 # 示例代码
│   ├── input_capture_demo.rs    # 输入捕获示例
│   ├── input_injection_demo.rs  # 输入注入示例
│   ├── permissions_demo.rs      # 权限检查示例
│   └── state_management_demo.rs # 状态管理示例
│
├── scripts/                  # 构建和工具脚本
│   ├── build_macos.sh       # macOS 一键打包脚本
│   ├── build_windows.ps1    # Windows 一键打包脚本
│   └── README.md            # 脚本使用说明
│
├── tests/                    # 测试套件
│   ├── property_tests.rs         # 属性测试
│   ├── e2e_integration_tests.rs  # 端到端集成测试
│   └── performance_tests.rs      # 性能基准测试
│
├── icons/                    # 应用图标资源
│   ├── icon.png             # 主图标
│   ├── 32x32.png            # 32x32 图标
│   ├── 128x128.png          # 128x128 图标
│   ├── 128x128@2x.png       # 高分辨率图标
│   └── README.md            # 图标说明
│
├── .github/                  # GitHub 配置
│   └── workflows/           # CI/CD 工作流
│
├── .kiro/                    # Kiro IDE 配置
│
├── Cargo.toml               # Rust 项目配置
├── Cargo.lock               # 依赖锁定文件
├── tauri.conf.json          # Tauri 应用配置
├── build.rs                 # 构建脚本
├── signing.example.json     # 代码签名配置示例
│
└── README.md                # 项目说明文档
```

### 构建输出

```
target/release/
├── kvm          # 命令行版本 (~2.5MB)
└── kvm-ui       # Tauri GUI 版本 (~9MB)

dist/           # 打包输出目录
├── Cross-Platform KVM.app              # macOS 应用包
├── Cross-Platform KVM_0.1.0_*.dmg      # macOS 安装镜像
├── Cross-Platform KVM-*-portable.zip   # Windows 便携版
└── Cross-Platform KVM-*.msi            # Windows 安装程序（可选）
```

### 前端页面结构

UI 采用多页面设计，每个功能独立页面：

- `index.html` - 主控制面板（设备列表、连接状态）
- `layout.html` - 设备布局配置（拖拽式布局编辑器）
- `hotkey.html` - 热键配置（快捷键绑定）
- `onboarding.html` - 新手引导（首次使用向导）
- `update.html` - 更新检查（版本更新管理）
- `permissions-check.html` - 权限检查（系统权限验证）

---

## 🧪 测试

项目包含全面的测试套件：

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test --test property_tests       # 属性测试
cargo test --test e2e_integration_tests # 集成测试
cargo test --test performance_tests     # 性能测试

# 运行测试并显示输出
cargo test -- --nocapture
```

---

## 🔒 安全模型

- **TLS 1.3 加密**: 所有设备通信使用 TLS 1.3 加密
- **Ed25519 密钥对**: 每个设备使用 Ed25519 密钥对
- **授权机制**: 设备 ID + 公钥指纹验证
- **输入验证**: 坐标边界检查、按键代码验证

---

## 🎯 性能目标

- 输入延迟：< 50ms (P99)
- 剪切板同步：< 500ms
- 网络带宽：< 1 Mbps（正常使用）
- CPU 使用率：< 5%
- 内存使用：< 100MB

---

## 🗺️ 路线图

### v0.1.0（当前版本）
- ✅ 基础框架和架构
- ✅ Tauri GUI 集成
- ✅ 设备发现机制
- ✅ 一键打包脚本
- � 输入捕获和注入（开发中）

### v0.2.0（计划中）
- [ ] 完整的鼠标和键盘控制
- [ ] 剪切板同步
- [ ] 屏幕边缘切换
- [ ] 稳定的跨设备通信

### v0.3.0（计划中）
- [ ] Linux 支持
- [ ] 文件传输功能
- [ ] 性能优化
- [ ] UI/UX 改进

---

## 📄 许可证

MIT License - 详见 LICENSE 文件

---

## 🙏 贡献

欢迎贡献！由于项目处于早期开发阶段，请先通过 Issue 讨论您想要添加的功能。

---

**⚡ 开箱即用，一键打包！** 🎉
