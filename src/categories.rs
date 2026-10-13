//! パーティションラベルに含まれる分類名から、既存の11種ICOと説明を対応付ける。
//! カテゴリが見つからない場合、勝手にドライブ文字から用途を推測しない。

pub struct Category {
    pub name: &'static str,
    pub description: &'static str,
    pub tags: &'static [&'static str],
    pub icon: &'static [u8],
}

pub static CATEGORIES: &[Category] = &[
    Category {
        name: "システム",
        description: "Windows・起動関連・ドライバー・復旧環境など、OSの動作に必要な領域。",
        tags: &["Windows", "起動", "ドライバー", "復旧"],
        icon: include_bytes!("../assets/01_システム.ico"),
    },
    Category {
        name: "アプリ",
        description: "インストールソフト・ゲーム・プラグイン・仮想環境とアプリ設定。",
        tags: &["アプリ", "ゲーム", "プラグイン", "環境"],
        icon: include_bytes!("../assets/02_アプリ.ico"),
    },
    Category {
        name: "データ",
        description: "文書・表計算・DB・写真・個人ファイルなどの汎用データを保管。",
        tags: &["文書", "DB", "写真", "個人データ"],
        icon: include_bytes!("../assets/03_データ.ico"),
    },
    Category {
        name: "作業",
        description: "進行中の案件・一時ファイル・ダウンロード・中間成果物を整理。",
        tags: &["進行中", "一時保管", "DL", "中間成果"],
        icon: include_bytes!("../assets/04_作業.ico"),
    },
    Category {
        name: "制作",
        description: "デザイン・イラスト・動画・音声・3D素材と書き出し成果物。",
        tags: &["デザイン", "映像", "音声", "3D", "素材"],
        icon: include_bytes!("../assets/05_制作.ico"),
    },
    Category {
        name: "開発",
        description: "ソースコード・Git・SDK・コンテナ・ビルド成果物とテスト。",
        tags: &["コード", "Git", "SDK", "ビルド", "テスト"],
        icon: include_bytes!("../assets/06_開発.ico"),
    },
    Category {
        name: "管理",
        description: "契約・経理・資産台帳・各種申請・設定バックアップなどを管理。",
        tags: &["契約", "経理", "資産", "申請", "設定"],
        icon: include_bytes!("../assets/07_管理.ico"),
    },
    Category {
        name: "参考",
        description: "電子書籍・論文・マニュアル・教材・辞書・調査資料を収集。",
        tags: &["書籍", "論文", "教材", "辞書", "資料"],
        icon: include_bytes!("../assets/08_参考.ico"),
    },
    Category {
        name: "档案",
        description: "完了案件・履歴・旧版・長期保存データ・バックアップを収蔵。",
        tags: &["アーカイブ", "履歴", "旧版", "長期保存"],
        icon: include_bytes!("../assets/09_档案.ico"),
    },
    Category {
        name: "生活",
        description: "家族写真・家計・旅行・健康記録・日常の各種書類を整理。",
        tags: &["家族", "家計", "旅行", "暮らし"],
        icon: include_bytes!("../assets/10_生活.ico"),
    },
    Category {
        name: "愉楽",
        description: "音楽・映画・ゲーム・趣味作品・娯楽用コレクションを保管。",
        tags: &["音楽", "映画", "ゲーム", "趣味"],
        icon: include_bytes!("../assets/11_愉楽.ico"),
    },
];

/// カテゴリの含有判定は元のボリュームラベルに対して実施する。
pub fn identify(label: &str) -> Option<&'static Category> {
    // 同義語を先に正規化して誤判定を抑える。
    if label.contains("アーカイブ") || label.contains("保管庫") {
        return CATEGORIES.iter().find(|c| c.name == "档案");
    }
    CATEGORIES.iter().find(|c| label.contains(c.name))
}

pub fn is_local_badge(label: &str) -> bool {
    label.contains("C2S｜") || label.contains("C2S|") || label.contains("C2S│")
}

/// 一致する分類名があれば余計な装飾を除き分類名のみ表示する。
/// 未分類の場合は C2S 接頭辞と見た目上の区切りのみ取り除く。
pub fn display_name(label: &str) -> String {
    if let Some(category) = identify(label) {
        return category.name.to_owned();
    }
    let cleaned = label
        .trim()
        .trim_start_matches("C2S｜")
        .trim_start_matches("C2S|")
        .trim_start_matches("C2S│")
        .trim()
        .trim_matches(|c: char| matches!(c, '｜' | '|' | '│' | '・' | '-' | '—' | ' '));
    if cleaned.is_empty() { "ローカルディスク".to_owned() } else { cleaned.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_and_badge() {
        assert_eq!(display_name("C2S｜データ"), "データ");
        assert!(is_local_badge("C2S｜データ"));
        assert!(!is_local_badge("通常のデータ"));
        assert_eq!(display_name("プロジェクト／制作 2026"), "制作");
        assert_eq!(display_name("C2S｜"), "ローカルディスク");
    }

    #[test]
    fn category_tags_up_to_five() {
        assert_eq!(CATEGORIES.len(), 11);
        assert!(CATEGORIES.iter().all(|c| c.tags.len() <= 5));
    }
}
