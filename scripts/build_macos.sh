#!/bin/bash
# ==============================================================================
# 破壁者 (Cross-Platform KVM) - macOS 一键打包脚本
# ==============================================================================
# 用法: ./scripts/build_macos.sh [--debug]
# 
# 此脚本会自动完成以下步骤：
# 1. 检查并安装必要的依赖
# 2. 构建 Release 版本
# 3. 创建 .app 应用包
# 4. 生成 .dmg 安装镜像
# ==============================================================================

set -e  # 遇到错误立即退出

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 项目配置
APP_NAME="Cross-Platform KVM"
APP_IDENTIFIER="com.kvm.cross-platform"
VERSION="0.1.0"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# 解析参数
BUILD_TYPE="release"
if [[ "$1" == "--debug" ]]; then
    BUILD_TYPE="debug"
fi

# 辅助函数
print_header() {
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

print_step() {
    echo -e "${GREEN}▶ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_error() {
    echo -e "${RED}✖ $1${NC}"
}

print_success() {
    echo -e "${GREEN}✔ $1${NC}"
}

# 检查命令是否存在
check_command() {
    if ! command -v "$1" &> /dev/null; then
        return 1
    fi
    return 0
}

# ==============================================================================
# 步骤1: 检查依赖
# ==============================================================================
print_header "步骤 1/4: 检查依赖"

# 检查 Rust
print_step "检查 Rust..."
if ! check_command rustc; then
    print_error "未找到 Rust。正在安装..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi
RUST_VERSION=$(rustc --version | awk '{print $2}')
print_success "Rust 版本: $RUST_VERSION"

# 检查 Cargo
print_step "检查 Cargo..."
if ! check_command cargo; then
    print_error "未找到 Cargo，请重新安装 Rust"
    exit 1
fi
print_success "Cargo 已安装"

# 检查 Xcode Command Line Tools
print_step "检查 Xcode Command Line Tools..."
if ! xcode-select -p &> /dev/null; then
    print_warning "正在安装 Xcode Command Line Tools..."
    xcode-select --install
    echo "请完成安装后重新运行此脚本"
    exit 1
fi
print_success "Xcode Command Line Tools 已安装"

# 检查 create-dmg (可选)
print_step "检查 create-dmg..."
if ! check_command create-dmg; then
    print_warning "create-dmg 未安装，将使用 hdiutil 创建 DMG"
    USE_CREATE_DMG=false
else
    print_success "create-dmg 已安装"
    USE_CREATE_DMG=true
fi

# ==============================================================================
# 步骤2: 构建项目
# ==============================================================================
print_header "步骤 2/4: 构建项目"

cd "$PROJECT_DIR"

print_step "清理旧构建..."
rm -rf "$PROJECT_DIR/dist"
mkdir -p "$PROJECT_DIR/dist"

print_step "构建 $BUILD_TYPE 版本..."
if [[ "$BUILD_TYPE" == "release" ]]; then
    cargo build --release
    BUILD_DIR="$PROJECT_DIR/target/release"
else
    cargo build
    BUILD_DIR="$PROJECT_DIR/target/debug"
fi

if [[ ! -f "$BUILD_DIR/kvm-ui" ]]; then
    print_error "构建失败：未找到 kvm-ui 可执行文件"
    exit 1
fi

print_success "构建完成"

# ==============================================================================
# 步骤3: 创建 .app 包
# ==============================================================================
print_header "步骤 3/4: 创建 .app 应用包"

APP_BUNDLE="$PROJECT_DIR/dist/${APP_NAME}.app"
CONTENTS_DIR="$APP_BUNDLE/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"

print_step "创建 .app 目录结构..."
mkdir -p "$MACOS_DIR"
mkdir -p "$RESOURCES_DIR"

print_step "复制可执行文件..."
cp "$BUILD_DIR/kvm-ui" "$MACOS_DIR/${APP_NAME}"
chmod +x "$MACOS_DIR/${APP_NAME}"

print_step "复制图标..."
if [[ -f "$PROJECT_DIR/icons/icon.icns" ]]; then
    cp "$PROJECT_DIR/icons/icon.icns" "$RESOURCES_DIR/icon.icns"
else
    print_warning "未找到图标文件，跳过图标复制"
fi

print_step "创建 Info.plist..."
cat > "$CONTENTS_DIR/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>CFBundleIdentifier</key>
    <string>${APP_IDENTIFIER}</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.utilities</string>
</dict>
</plist>
EOF

print_success ".app 包创建完成: $APP_BUNDLE"

# ==============================================================================
# 步骤4: 创建 DMG 镜像
# ==============================================================================
print_header "步骤 4/4: 创建 DMG 安装镜像"

# 获取架构
ARCH=$(uname -m)
if [[ "$ARCH" == "arm64" ]]; then
    ARCH_NAME="aarch64"
else
    ARCH_NAME="x64"
fi

DMG_NAME="${APP_NAME}_${VERSION}_${ARCH_NAME}.dmg"
DMG_PATH="$PROJECT_DIR/dist/$DMG_NAME"

print_step "创建 DMG..."

if [[ "$USE_CREATE_DMG" == true ]]; then
    # 使用 create-dmg 创建更美观的 DMG
    create-dmg \
        --volname "${APP_NAME}" \
        --volicon "$PROJECT_DIR/icons/icon.icns" \
        --window-pos 200 120 \
        --window-size 600 400 \
        --icon-size 100 \
        --icon "${APP_NAME}.app" 150 185 \
        --hide-extension "${APP_NAME}.app" \
        --app-drop-link 450 185 \
        --no-internet-enable \
        "$DMG_PATH" \
        "$APP_BUNDLE" || true
else
    # 使用 hdiutil 创建简单 DMG
    # 创建临时目录
    TEMP_DMG_DIR="$PROJECT_DIR/dist/dmg_temp"
    mkdir -p "$TEMP_DMG_DIR"
    cp -R "$APP_BUNDLE" "$TEMP_DMG_DIR/"
    
    # 创建 Applications 快捷方式
    ln -sf /Applications "$TEMP_DMG_DIR/Applications"
    
    # 创建 DMG
    hdiutil create -volname "${APP_NAME}" \
        -srcfolder "$TEMP_DMG_DIR" \
        -ov -format UDZO \
        "$DMG_PATH"
    
    # 清理
    rm -rf "$TEMP_DMG_DIR"
fi

if [[ -f "$DMG_PATH" ]]; then
    print_success "DMG 创建完成: $DMG_PATH"
else
    print_warning "DMG 创建可能未成功，请检查输出"
fi

# ==============================================================================
# 完成
# ==============================================================================
print_header "打包完成！"

echo ""
echo -e "${GREEN}输出文件:${NC}"
echo "  📦 应用包: $APP_BUNDLE"
if [[ -f "$DMG_PATH" ]]; then
    echo "  💿 安装镜像: $DMG_PATH"
    DMG_SIZE=$(du -h "$DMG_PATH" | cut -f1)
    echo "  📊 DMG 大小: $DMG_SIZE"
fi

echo ""
echo -e "${YELLOW}安装说明:${NC}"
echo "  1. 双击打开 DMG 文件"
echo "  2. 将 ${APP_NAME} 拖入 Applications 文件夹"
echo "  3. 首次运行需要授予辅助功能权限"
echo ""
echo -e "${GREEN}✔ 一键打包完成！${NC}"
