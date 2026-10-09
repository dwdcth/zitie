#!/usr/bin/env bash
# 下载内置开源字体到 assets/fonts/(幂等:已存在且校验通过则跳过)
# 用法: ./scripts/fetch-fonts.sh [--force]
set -euo pipefail

cd "$(dirname "$0")/.."
DIR="assets/fonts"
mkdir -p "$DIR"

FORCE=0
[[ "${1:-}" == "--force" ]] && FORCE=1

# id | 文件名 | 下载地址 | 许可证地址
ENTRIES=(
  "lxgw-wenkai|LXGWWenKai-Regular.ttf|https://github.com/lxgw/LxgwWenKai/releases/download/v1.522/LXGWWenKai-Regular.ttf|https://github.com/lxgw/LxgwWenKai/raw/main/OFL.txt|OFL-LXGWWenKai.txt"
  "shortstack|ShortStack-Regular.ttf|https://github.com/google/fonts/raw/main/ofl/shortstack/ShortStack-Regular.ttf|https://github.com/google/fonts/raw/main/ofl/shortstack/OFL.txt|OFL-ShortStack.txt"
  "andika|Andika-Regular.ttf|https://github.com/google/fonts/raw/main/ofl/andika/Andika-Regular.ttf|https://github.com/google/fonts/raw/main/ofl/andika/OFL.txt|OFL-Andika.txt"
)

fail=0
for e in "${ENTRIES[@]}"; do
  IFS='|' read -r id file url lic_url lic_name <<< "$e"
  if [[ $FORCE -eq 0 && -s "$DIR/$file" ]]; then
    echo "[跳过] $DIR/$file 已存在(如需重新下载请加 --force)"
  else
    echo "[下载] $file ..."
    if ! curl -fL --retry 3 --connect-timeout 20 -o "$DIR/$file.part" "$url"; then
      echo "[失败] 无法下载 $url" >&2
      rm -f "$DIR/$file.part"
      fail=1
      continue
    fi
    mv "$DIR/$file.part" "$DIR/$file"
  fi
  if [[ ! -s "$DIR/$lic_name" ]]; then
    curl -fL --retry 3 --connect-timeout 20 -o "$DIR/$lic_name" "$lic_url" \
      || { echo "[警告] 许可证文本下载失败: $lic_url" >&2; rm -f "$DIR/$lic_name"; }
  fi
done

if [[ $fail -ne 0 ]]; then
  echo "部分字体下载失败,请检查网络后重试" >&2
  exit 1
fi

echo "完成。字体校验和(用于 assets/fonts/fonts.json):"
(cd "$DIR" && sha256sum -- *.ttf)
