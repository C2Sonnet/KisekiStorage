//! ボリューム名による分類。ドライブ文字で用途を推測しない。
//! 説明・タグは個人情報や固有の保存先名を含めない公開向け概要。

pub struct Category {
    pub name: &'static str,
    pub group: &'static str,
    pub description: &'static str,
    pub tags: &'static [&'static str],
    pub icon: &'static [u8],
}

pub const GROUPS: &[&str] = &["共通", "生活相関", "作業相関", "仕事相関", "アーカイブ", "未分類"];

pub static CATEGORIES: &[Category] = &[
    Category {
        name: "システム", group: "共通",
        description: "Windows・起動設定・ドライバー・回復環境など、PCを動かすためのシステム領域。",
        tags: &["OS", "起動", "ドライバー", "復旧"],
        icon: include_bytes!("../assets/partitions/システム.png"),
    },
    Category {
        name: "アプリ", group: "共通",
        description: "インストール済みソフト・プラグイン・各種ツール・アプリの実行環境を保存。",
        tags: &["ソフト", "ツール", "プラグイン", "環境"],
        icon: include_bytes!("../assets/partitions/アプリ.png"),
    },
    Category {
        name: "データ", group: "共通",
        description: "各種ドキュメント・写真・表計算・データベースなどの汎用ファイルを保管。",
        tags: &["文書", "写真", "DB", "汎用データ"],
        icon: include_bytes!("../assets/partitions/データ.png"),
    },
    Category {
        name: "日常", group: "生活相関",
        description: "日々の写真と暮らしの計画を整理。家計・旅行・イベントなどの予定と記録をまとめる。",
        tags: &["写真", "家計", "旅行", "イベント"],
        icon: include_bytes!("../assets/partitions/日常.png"),
    },
    Category {
        name: "档案", group: "生活相関",
        description: "健康・医療の記録、学歴・職歴・住居の履歴、行政・経済・口座関係の書類を整理。",
        tags: &["記録", "履歴", "証明書", "行政", "経済"],
        icon: include_bytes!("../assets/partitions/档案.png"),
    },
    Category {
        name: "愉楽", group: "生活相関",
        description: "音楽・ゲーム・アニメ・漫画・小説・ドラマ・テレビ録画・ライブなどの娯楽作品を保管。",
        tags: &["音楽", "ゲーム", "映像", "読書", "ライブ"],
        icon: include_bytes!("../assets/partitions/愉楽.png"),
    },
    Category {
        name: "研究", group: "作業相関",
        description: "自然・人文・社会・総合科学の探究、AI技術の調査、自由研究と研究ノートを整理。",
        tags: &["科学", "人文", "社会", "AI", "調査"],
        icon: include_bytes!("../assets/partitions/研究.png"),
    },
    Category {
        name: "芸術", group: "作業相関",
        description: "小説・漫画・イラスト・音楽・映像・デザイン・工作・折り紙・3Dモデルなどの創作素材と成果物。",
        tags: &["創作", "デザイン", "音楽", "映像", "3D"],
        icon: include_bytes!("../assets/partitions/芸術.png"),
    },
    Category {
        name: "応用", group: "作業相関",
        description: "PCソフト・Webアプリ・サイト・統合システム・各種ゲームの設計、コード、制作物を管理。",
        tags: &["プログラム", "Web", "システム", "ゲーム"],
        icon: include_bytes!("../assets/partitions/応用.png"),
    },
    Category {
        name: "工程", group: "作業相関",
        description: "PC電子機器・USB機器などのハードウェア開発と、住宅・事務所などの建築計画を保存。",
        tags: &["電子", "ハード", "USB", "建築", "設計"],
        icon: include_bytes!("../assets/partitions/工程.png"),
    },
    Category {
        name: "仕事", group: "仕事相関",
        description: "会社・自営業・アルバイトの業務資料と、経営・市場調査に関するファイルをまとめる。",
        tags: &["業務", "雇用", "経営", "市場調査"],
        icon: include_bytes!("../assets/partitions/仕事.png"),
    },
    Category {
        name: "投資", group: "仕事相関",
        description: "市場の分析、投資方針や計画、運用記録、資産管理の資料を整理。",
        tags: &["市場", "分析", "投資計画", "運用", "資産"],
        icon: include_bytes!("../assets/partitions/投資.png"),
    },
    Category {
        name: "参考", group: "アーカイブ",
        description: "調査・制作・学習に役立つ参考図書や資料、過去の収集物を長期的に保存するアーカイブ。",
        tags: &["参考資料", "書籍", "学習", "長期保存"],
        icon: include_bytes!("../assets/partitions/参考.png"),
    },
];

pub fn identify(label: &str) -> Option<&'static Category> {
    // 正式な名称が含まれているかを優先し、同じ意味とは限らない語を勝手に置換しない。
    CATEGORIES.iter().find(|c| label.contains(c.name))
}

pub fn group_for(label: &str) -> &'static str {
    identify(label).map_or("未分類", |category| category.group)
}

pub fn is_local_badge(label: &str) -> bool {
    label.contains("C2S｜") || label.contains("C2S|") || label.contains("C2S│")
}

pub fn display_name(label: &str) -> String {
    if let Some(category) = identify(label) { return category.name.to_owned(); }
    let cleaned = label.trim()
        .trim_start_matches("C2S｜").trim_start_matches("C2S|").trim_start_matches("C2S│")
        .trim().trim_matches(|c: char| matches!(c, '｜' | '|' | '│' | '・' | '-' | '—' | ' '));
    if cleaned.is_empty() { "ローカルディスク".into() } else { cleaned.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn category_mapping_uses_label_not_drive() {
        assert_eq!(display_name("C2S｜システム"), "システム");
        assert!(is_local_badge("C2S｜システム"));
        assert_eq!(group_for("C2S｜日常"), "生活相関");
        assert_eq!(group_for("C2S｜研究"), "作業相関");
        assert_eq!(group_for("C2S｜投資"), "仕事相関");
        assert_eq!(group_for("C2S｜参考"), "アーカイブ");
        assert_eq!(group_for("C2S｜見慣れない名前"), "未分類");
        assert!(!is_local_badge("参考"));
    }
    #[test]
    fn all_icons_present_and_tags_valid() {
        assert_eq!(CATEGORIES.len(), 13);
        assert!(CATEGORIES.iter().all(|c| !c.icon.is_empty() && c.tags.len() <= 5));
    }
}
