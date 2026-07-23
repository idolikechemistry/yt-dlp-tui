#!/usr/bin/env bash
# =====================================================================
# yt-dlp-tui - macOS & Linux 一鍵自動化安裝/升級指令碼
# =====================================================================
# 支援系統：macOS (Apple Silicon M1/M2/M3/M4) & Linux (x86_64)
# 運作特點：
# 1. 自動偵測處理器架構（相容 Rosetta 2 轉譯環境）。
# 2. 對齊 v20260723.0.3 精簡化資產命名（mac-arm64, linux-x64）。
# 3. 提供原子化部署：自動備份舊版本，若安裝中斷或失敗，自動安全回滾。
# 4. macOS 平台自動清除 Gatekeeper 隔離標記（com.apple.quarantine）。
# =====================================================================

set -eo pipefail

# 1. 偵測系統環境與架構
OS="$(uname -s)"
ARCH="$(uname -m)"
PLATFORM=""

if [ "$OS" = "Darwin" ]; then
    # 針對 Apple Silicon M1/M2/M3/M4 (原生 arm64 或是處於 Rosetta 2 轉譯模式)
    if [ "$ARCH" = "arm64" ] || [ "$(sysctl -in sysctl.proc_translated 2>/dev/null)" = "1" ]; then
        PLATFORM="mac-arm64"
    else
        echo "❌ [錯誤] 僅支援 Apple Silicon (M1/M2/M3/M4) 架構的 macOS 系統。"
        exit 1
    fi
elif [ "$OS" = "Linux" ]; then
    if [ "$ARCH" = "x86_64" ]; then
        PLATFORM="linux-x64"
    else
        echo "❌ [錯誤] Linux 平台目前僅支援 x86_64 架構。"
        exit 1
    fi
else
    echo "❌ [錯誤] 不支援的作業系統: $OS"
    exit 1
fi

# 2. 設定下載與部署路徑參數
REPO="idolikechemistry/yt-dlp-tui"
FILE_NAME="yt-dlp-tui-${PLATFORM}.tar.gz"
URL="https://github.com/${REPO}/releases/latest/download/${FILE_NAME}"
INSTALL_DIR="/usr/local/bin"
TARGET_PATH="${INSTALL_DIR}/yt-dlp-tui"
BACKUP_PATH="${TARGET_PATH}.bak"

echo "--------------------------------------------------"
echo "ℹ️  偵測到作業平台：${PLATFORM}"
echo "ℹ️  目標安裝路徑：${TARGET_PATH}"
echo "--------------------------------------------------"

# 3. 檢查寫入權限，自適應調用 sudo
SUDO=""
if [ ! -w "$INSTALL_DIR" ]; then
    echo "⚠️  偵測到 ${INSTALL_DIR} 需要管理者寫入權限，將調用 sudo 執行。"
    SUDO="sudo"
fi

# 4. 建立隔離的系統暫存資料夾
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

# 5. 下載與解壓縮資產
echo "📥 正在自 GitHub 下載最新編譯產物..."
if ! curl -fsSL "$URL" -o "${TMP_DIR}/${FILE_NAME}"; then
    echo "❌ [錯誤] 下載失敗，請檢查網路連線或該版本資產是否已完成編譯發布。"
    exit 1
fi

echo "📦 正在解壓縮檔案..."
if ! tar -xzf "${TMP_DIR}/${FILE_NAME}" -C "$TMP_DIR"; then
    echo "❌ [錯誤] 解壓縮資產時發生損毀或錯誤。"
    exit 1
fi

# 確保解壓後的執行檔存在
EXTRACTED_BIN="${TMP_DIR}/yt-dlp-tui"
if [ ! -f "$EXTRACTED_BIN" ]; then
    echo "❌ [錯誤] 解壓後的歸檔中未包含執行檔 'yt-dlp-tui'。"
    exit 1
fi

# 6. 原子化部署與安全備份回滾
if [ -f "$TARGET_PATH" ]; then
    echo "💾 偵測到現有舊版本，正在建立安全備份..."
    if ! $SUDO mv "$TARGET_PATH" "$BACKUP_PATH"; then
        echo "❌ [錯誤] 無法備份舊版本，安裝程序終止。"
        exit 1
    fi
fi

echo "🚀 正在將全新二進位檔部署至系統路徑..."
if ! $SUDO mv "$EXTRACTED_BIN" "$TARGET_PATH"; then
    echo "❌ [錯誤] 部署新版執行檔失敗！"
    if [ -f "$BACKUP_PATH" ]; then
        echo "🔄 正在將舊版本安全還原..."
        $SUDO mv "$BACKUP_PATH" "$TARGET_PATH"
    fi
    exit 1
fi

# 7. 權限設定與 macOS 安全隔離解除
echo "🔑 正在調整檔案可執行權限..."
$SUDO chmod +x "$TARGET_PATH"

if [ "$OS" = "Darwin" ]; then
    echo "🛡️  正在為 macOS 清除 Gatekeeper 隔離標記..."
    # 忽略找不到標記的非致命錯誤
    $SUDO xattr -d com.apple.quarantine "$TARGET_PATH" 2>/dev/null || true
fi

# 8. 安全清理備份檔案
if [ -f "$BACKUP_PATH" ]; then
    $SUDO rm -f "$BACKUP_PATH"
fi

# 9. 自我功能驗證
echo "--------------------------------------------------"
if command -v yt-dlp-tui >/dev/null 2>&1; then
    INSTALLED_VER="$(yt-dlp-tui -V 2>/dev/null || yt-dlp-tui --version 2>/dev/null || echo "未知版本")"
    echo "✨ [成功] yt-dlp-tui 一鍵部署與升級已順利完成！"
    echo "🎉 目前系統中安裝的版本為：${INSTALLED_VER}"
else
    echo "⚠️  [警告] 部署成功但未能在系統 PATH 環境變數中定位 yt-dlp-tui。"
    echo "👉 請確保您的系統環境變數包含 ${INSTALL_DIR} 目錄。"
fi
echo "--------------------------------------------------"
