#!/bin/bash

# =====================================================================
# yt-dlp-tui 跨平台智慧發布指令碼 (V3 - 支援 Pre-release 與分支智慧判定)
# =====================================================================

# 確保指令失敗時立刻中斷
set -e

# 定義顯示樣式 (相容無特殊字元終端機)
info() { echo "[Info] $1"; }
warn() { echo "[Warning] $1"; }
err() { echo "[Error] $1"; exit 1; }
success() { echo "[Success] $1"; }

# 1. 執行前置本地 Rust 代碼檢查 (cargo check)
info "正在執行前置編譯安全性檢查 (cargo check)..."
if ! cargo check; then
    err "本地代碼編譯檢查失敗！請修復錯誤後再嘗試發布。"
fi
success "本地代碼檢查通過！"

# 2. 獲取當前 Git 分支與 Cargo.toml 中的版本號
CURRENT_BRANCH=$(git branch --show-current 2>/dev/null || echo "unknown")
VER=$(grep -m 1 '^version' Cargo.toml | cut -d '"' -f 2)

if [ -z "$VER" ]; then
    err "無法從 Cargo.toml 中解析出版本號 (version)。"
fi

# 3. 智慧判定版本類型與分支相容性
IS_PRERELEASE=false
if [[ "$VER" == *"-"* ]]; then
    IS_PRERELEASE=true
fi

info "偵測到目前 Git 分支：$CURRENT_BRANCH"
info "偵測到 Cargo.toml 版本號：$VER (Pre-release: $IS_PRERELEASE)"

# 智慧防錯警告邏輯
if [ "$CURRENT_BRANCH" = "main" ] || [ "$CURRENT_BRANCH" = "master" ]; then
    if [ "$IS_PRERELEASE" = true ]; then
        warn "您目前在主分支 ($CURRENT_BRANCH)，但 Cargo.toml 中的版本號卻是預發布格式 ($VER)。"
        read -p "確定要在主分支發布 Pre-release 版本嗎？(y/N) " confirm
        if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
            err "已取消發布。"
        fi
    fi
else
    # 在非主分支 (例如 dev)
    if [ "$IS_PRERELEASE" = false ]; then
        warn "您目前在開發分支 ($CURRENT_BRANCH)，但 Cargo.toml 中的版本號是正式版格式 ($VER)。"
        echo "💡 提示：建議在開發分支使用 pre-release 命名格式 (例如 $VER-beta.1)；"
        echo "   或者切換至 main 分支再發布正式版本。"
        read -p "您確定要直接在開發分支發布正式版嗎？(y/N) " confirm
        if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
            err "已取消發布。請先修改 Cargo.toml 的版本號或切換 Git 分支。"
        fi
    fi
fi

TAG="v$VER"

# 4. 檢查 Tag 是否已存在 (本地與遠端)
if git rev-parse "$TAG" >/dev/null 2>&1; then
    err "本地已存在標籤 $TAG，請先在 Cargo.toml 中增加版本號！"
fi

# 5. 處理工作區未提交的修改 (Dirty Working Tree)
if ! git diff-index --quiet HEAD --; then
    warn "偵測到您的工作區有未提交的代碼修改："
    git status -s
    echo ""
    read -p "是否自動將這些修改併入本次發布的 Commit 中？(y/N) " confirm
    if [[ "$confirm" =~ ^[Yy]$ ]]; then
        info "正在暫存並提交修改..."
        git add .
        git commit -m "chore: release $TAG ($CURRENT_BRANCH)"
    else
        err "發布已中斷。請先手動處理您的未提交變更 (Git Commit) 後再執行本指令碼。"
    fi
else
    # 工作區乾淨，但可能修改了 Cargo.toml 還未 commit
    info "工作區狀態乾淨，正在為發布建立 Commit..."
    # 建立一個空 commit 或自動 add Cargo.toml/Cargo.lock 確保版本號同步
    git add Cargo.toml Cargo.lock 2>/dev/null || true
    git commit -m "chore: bump version to $TAG ($CURRENT_BRANCH)" 2>/dev/null || info "無新變更需要 Commit，將直接打上 Tag。"
fi

# 6. 本地建立 Tag
info "正在建立本地標籤：$TAG..."
git tag -a "$TAG" -m "Release $TAG: 於分支 $CURRENT_BRANCH 自動建構發布"

# 7. 推送至 GitHub (包含程式碼與 Tag)
info "正在推送代碼至遠端倉庫 ($CURRENT_BRANCH)..."
if ! git push origin "$CURRENT_BRANCH"; then
    git tag -d "$TAG" # 發生錯誤時本地回滾刪除 Tag
    err "代碼推送失敗！已自動在本地刪除 $TAG 標籤以保持狀態乾淨。"
fi

info "正在推送標籤至遠端倉庫 ($TAG)..."
if ! git push origin "$TAG"; then
    git tag -d "$TAG"
    # 嘗試刪除 GitHub 上可能推送失敗但產生殘留的遠端標籤 (防護)
    git push origin --delete "$TAG" >/dev/null 2>&1 || true
    err "標籤推送失敗！已自動在本地刪除 $TAG 標籤以保持狀態乾淨。"
fi

# 8. 成功總結
echo ""
echo "====================================================================="
if [ "$IS_PRERELEASE" = true ]; then
    success "一鍵發布指令執行完畢！"
    info "GitHub Actions 偵測到版本號含有 '-'，將會自動建構為 [Pre-release (預發布)] 版本！"
else
    success "一鍵發布指令執行完畢！"
    info "GitHub Actions 將會自動建構為 [Latest (正式發行)] 版本！"
fi
info "請前往您的 GitHub Repository -> Actions 查看自動化跨平台編譯進度。"
echo "====================================================================="
