#!/usr/bin/env bash
set -e

# 1. 執行 cargo check 進行前置安全編譯檢查
echo "[Info] 正在執行 cargo check 進行前置編譯檢查..."
if ! cargo check; then
    echo "[Error] 本地代碼編譯失敗！請先修復錯誤再進行發布。"
    exit 1
fi
echo "[Success] 本地代碼編譯檢查通過！"

# 2. 讀取 Cargo.toml 內的第一個 version 號
VER=$(grep -m 1 '^version' Cargo.toml | cut -d '"' -f 2)
if [ -z "$VER" ]; then
    echo "[Error] 無法從 Cargo.toml 中讀取到版本號！"
    exit 1
fi
TAG="v$VER"
echo "[Info] 偵測到發布版本號為: $TAG"

# 檢查此 Tag 是否已在本地存在
if git rev-parse "$TAG" >/dev/null 2>&1; then
    echo "[Error] 標籤 $TAG 已在本地存在！請在 Cargo.toml 中更新版本號。"
    exit 1
fi

# 3. 檢查目前所在的分支與版號是否匹配
BRANCH=$(git branch --show-current)
echo "[Info] 目前所在分支為: $BRANCH"

# 判定是否為 beta/pre-release 版本 (版號中包含 '-')
IS_PRERELEASE=false
if [[ "$VER" == *"-"* ]]; then
    IS_PRERELEASE=true
fi

if [ "$BRANCH" = "dev" ] && [ "$IS_PRERELEASE" = "false" ]; then
    echo "[Warning] 您目前在 dev 分支，但準備發布正式版本號 ($VER)！"
    read -r -p "是否要繼續發布？(y/N): " CONFIRM
    if [[ ! "$CONFIRM" =~ ^[Yy]$ ]]; then
        echo "[Info] 發布已取消。"
        exit 0
    fi
elif [ "$BRANCH" = "main" ] && [ "$IS_PRERELEASE" = "true" ]; then
    echo "[Warning] 您目前在 main 分支，但準備發布預發布版本號 ($VER)！"
    read -r -p "是否要繼續發布？(y/N): " CONFIRM
    if [[ ! "$CONFIRM" =~ ^[Yy]$ ]]; then
        echo "[Info] 發布已取消。"
        exit 0
    fi
fi

# 4. 提示輸入自訂的版本標籤說明 (Tag Message)
echo ""
echo "=================================================="
echo "請輸入本次版本的更新說明 (Tag 說明)："
echo "例如：'修正 ui 對齊問題' 或 '新增自動黑名單防護與重試功能'"
echo "=================================================="
read -r -p "> " USER_DESCRIPTION

if [ -z "$USER_DESCRIPTION" ]; then
    TAG_MSG="Release $TAG: 效能優化與問題修正"
    echo "[Info] 未輸入說明，將使用預設值: '$TAG_MSG'"
else
    TAG_MSG="Release $TAG: $USER_DESCRIPTION"
fi
echo "[Info] 最終 Tag 說明將設定為: '$TAG_MSG'"
echo ""

# 5. 檢查是否有未提交的變更 (Dirty Workspace)
if ! git diff-index --quiet HEAD --; then
    echo "[Warning] 本地工作區有尚未提交的修改："
    git status -s
    echo ""
    read -r -p "是否自動將上述修改加入 Commit 並發布？(y/N): " CONFIRM_COMMIT
    if [[ "$CONFIRM_COMMIT" =~ ^[Yy]$ ]]; then
        git add .
        git commit -m "chore: release $TAG"
        echo "[Success] 已建立 Release Commit"
    else
        echo "[Info] 請手動提交或清理工作區後再執行發布。"
        exit 0
    fi
else
    echo "[Info] 本地工作區乾淨，直接進行發布程序。"
fi

# 6. 本地建立 Tag 並推送 (包含出錯回滾機制)
echo "[Info] 正在建立本地標籤 $TAG..."
git tag -a "$TAG" -m "$TAG_MSG"

echo "[Info] 正在推送到遠端倉庫..."
if git push origin "$BRANCH" && git push origin "$TAG"; then
    echo ""
    echo "=================================================="
    echo " [Success] 一鍵推送發布完成！"
    echo " 遠端分支：$BRANCH"
    echo " 發布標籤：$TAG ($TAG_MSG)"
    echo "=================================================="
    echo "提示：GitHub Actions 已被觸發，請前往 Actions 頁面查看編譯進度。"
else
    echo ""
    echo "[Error] 推送至 GitHub 失敗！"
    echo "正在回滾本地建立的標籤 $TAG，以保持本地與遠端同步..."
    git tag -d "$TAG"
    echo "[Info] 本地標籤 $TAG 已刪除。請確認您的網路連接、GitHub 權限，然後再試一次。"
    exit 1
fi
