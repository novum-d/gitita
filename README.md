# Qiita 自動投稿

## 概要

このリポジトリは、Git と CI を用いて Qiita 記事を管理します。

---

## フロー

1. articles/<slug>/article.md に記事を作成
2. PR を作成
3. レビュー
4. マージ
5. Qiita に自動投稿

---

## 記事フォーマット

```md
---
title: ""
tags: []
author: ""
qiita_id: null
---

本文
````

---

## 予定コマンド

```bash
cargo run -- check
cargo run -- publish --dry-run
```

`publish` 本体は Qiita API 連携の実装後に有効化します。

---

## 注意事項

* qiita_id は自動で管理されます
* qiita_id を手動で編集しないでください
