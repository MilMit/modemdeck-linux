use std::sync::{OnceLock, RwLock};

pub mod catalog;
pub mod validation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    En,
    Fa,
    Ar,
    Tr,
    De,
    Fr,
    Es,
    Ru,
}

static LANG: OnceLock<RwLock<Language>> = OnceLock::new();

pub fn init(value: &str) {
    let normalized = value.trim().to_ascii_lowercase();
    let code = if normalized == "system" || normalized.is_empty() {
        std::env::var("LANG")
            .unwrap_or_default()
            .to_ascii_lowercase()
    } else {
        normalized
    };

    let lang = if code.starts_with("fa") {
        Language::Fa
    } else if code.starts_with("ar") {
        Language::Ar
    } else if code.starts_with("tr") {
        Language::Tr
    } else if code.starts_with("de") {
        Language::De
    } else if code.starts_with("fr") {
        Language::Fr
    } else if code.starts_with("es") {
        Language::Es
    } else if code.starts_with("ru") {
        Language::Ru
    } else {
        Language::En
    };

    let lock = LANG.get_or_init(|| RwLock::new(lang));
    if let Ok(mut current) = lock.write() {
        *current = lang;
    }
}

pub fn language() -> Language {
    LANG.get_or_init(|| RwLock::new(Language::En))
        .read()
        .map(|v| *v)
        .unwrap_or(Language::En)
}

pub fn is_rtl() -> bool {
    matches!(language(), Language::Fa | Language::Ar)
}

pub fn tr(key: &str) -> &str {
    if let Some(found) = catalog::lookup(language(), key) {
        return found;
    }
    match language() {
        Language::En => en(key),
        Language::Fa => fa(key),
        Language::Ar => ar(key),
        Language::Tr => tr_lang(key),
        Language::De => de(key),
        Language::Fr => fr(key),
        Language::Es => es(key),
        Language::Ru => ru(key),
    }
}

fn en(key: &str) -> &str {
    match key {
        "preferences" => "Preferences",
        "supported_modems" => "Supported modems",
        "check_updates" => "Check for updates",
        "release_notes" => "Release notes",
        "report_problem" => "Report a problem",
        "apn_manager" => "APN manager",
        "sim_security" => "SIM security",
        "unlock_sim" => "Unlock SIM",
        "data_usage" => "Data usage & live traffic",
        "profiles" => "Band profiles",
        "restore_profile" => "Restore profile after reboot/resume",
        "save_profile" => "Save current selection as profile",
        "apply_profile" => "Apply profile",
        "delete_profile" => "Delete profile",
        "language" => "Language",
        "theme" => "Appearance theme",
        "system" => "System default",
        "dark" => "Dark",
        "light" => "Light",
        "english" => "English",
        "persian" => "فارسی (Persian)",
        "arabic" => "العربية (Arabic)",
        "turkish" => "Türkçe (Turkish)",
        "german" => "Deutsch (German)",
        "french" => "Français (French)",
        "spanish" => "Español (Spanish)",
        "russian" => "Русский (Russian)",
        "today" => "Today",
        "this_month" => "This month",
        "download" => "Download",
        "upload" => "Upload",
        "live_speed" => "Live speed",
        "reset_usage" => "Reset usage counters",
        "create_apn" => "Create APN profile",
        "activate" => "Activate",
        "delete" => "Delete",
        "edit" => "Edit",
        "save" => "Save",
        "cancel" => "Cancel",
        "about" => "About",
        "diagnostics" => "Export diagnostics",
        "refresh" => "Refresh modem data",
        "hotplug" => "Hot-plug ready",
        "suspend_resume" => "Suspend/resume aware",
        "crash_safe" => "Crash-safe rollback armed",
        "website" => "MilMit website",
        "band_analyzer" => "Band Analyzer",
        "ca_analyzer" => "Carrier Aggregation Analyzer",
        "speed_test" => "Internet Speed Test",
        "stability_monitor" => "Connection Stability Monitor",
        "operator_comparison" => "Operator Comparison",
        "mobile_hotspot" => "Mobile Hotspot",
        "persistent_lock" => "Persistent Band Lock",
        "smart_selection" => "Smart Band Selection",
        _ => key,
    }
}

/// Natural, everyday Persian (clear and easy to understand, avoiding overly academic terms).
fn fa(key: &str) -> &str {
    match key {
        "preferences" => "تنظیمات",
        "supported_modems" => "مودم‌های پشتیبانی‌شده",
        "check_updates" => "بررسی آپدیت",
        "release_notes" => "تغییرات نسخه",
        "report_problem" => "گزارش مشکل",
        "apn_manager" => "تنظیمات APN",
        "sim_security" => "قفل و امنیت سیم‌کارت",
        "unlock_sim" => "باز کردن قفل سیم‌کارت",
        "data_usage" => "مصرف اینترنت و سرعت لحظه‌ای",
        "profiles" => "پروفایل‌های باند",
        "restore_profile" => "برگرداندن پروفایل بعد از ری‌استارت",
        "save_profile" => "ذخیره این انتخاب به‌عنوان پروفایل",
        "apply_profile" => "اعمال پروفایل",
        "delete_profile" => "حذف پروفایل",
        "language" => "زبان برنامه",
        "theme" => "تم ظاهری",
        "system" => "پیش‌فرض سیستم",
        "dark" => "تیره",
        "light" => "روشن",
        "today" => "امروز",
        "this_month" => "این ماه",
        "download" => "دانلود",
        "upload" => "آپلود",
        "live_speed" => "سرعت لحظه‌ای",
        "reset_usage" => "صفر کردن آمار مصرف",
        "create_apn" => "ساخت APN جدید",
        "activate" => "فعال کردن",
        "delete" => "حذف",
        "edit" => "ویرایش",
        "save" => "ذخیره",
        "cancel" => "لغو",
        "about" => "درباره برنامه",
        "diagnostics" => "خروجی گزارش عیب‌یابی",
        "refresh" => "بروزرسانی اطلاعات مودم",
        "band_analyzer" => "تست و مقایسه باندها",
        "ca_analyzer" => "بررسی تجمیع باند (4G+)",
        "speed_test" => "تست سرعت اینترنت",
        "stability_monitor" => "بررسی پایداری اتصال",
        "operator_comparison" => "مقایسه اپراتورها",
        "mobile_hotspot" => "هات‌اسپات وای‌فای",
        "persistent_lock" => "قفل دائمی باند",
        "smart_selection" => "انتخاب خودکار بهترین باند",
        _ => en(key),
    }
}

fn ar(key: &str) -> &str {
    match key {
        "preferences" => "الإعدادات",
        "supported_modems" => "المودمات المدعومة",
        "check_updates" => "التحقق من التحديثات",
        "release_notes" => "ملاحظات الإصدار",
        "report_problem" => "الإبلاغ عن مشكلة",
        "language" => "اللغة",
        "theme" => "المظهر",
        "dark" => "داكن",
        "light" => "فاتح",
        "system" => "افتراضي النظام",
        "about" => "حول",
        "diagnostics" => "تصدير التشخيص",
        "refresh" => "تحديث بيانات المودم",
        "save" => "حفظ",
        "cancel" => "إلغاء",
        _ => en(key),
    }
}

fn tr_lang(key: &str) -> &str {
    match key {
        "preferences" => "Tercihler",
        "supported_modems" => "Desteklenen modemler",
        "check_updates" => "Güncellemeleri denetle",
        "release_notes" => "Sürüm notları",
        "report_problem" => "Sorun bildir",
        "language" => "Dil",
        "theme" => "Tema",
        "dark" => "Koyu",
        "light" => "Açık",
        "system" => "Sistem varsayılanı",
        "about" => "Hakkında",
        "diagnostics" => "Tanılama dışa aktar",
        "refresh" => "Modem verilerini yenile",
        "save" => "Kaydet",
        "cancel" => "İptal",
        _ => en(key),
    }
}

fn de(key: &str) -> &str {
    match key {
        "preferences" => "Einstellungen",
        "supported_modems" => "Unterstützte Modems",
        "check_updates" => "Nach Updates suchen",
        "release_notes" => "Versionshinweise",
        "report_problem" => "Problem melden",
        "language" => "Sprache",
        "theme" => "Design",
        "dark" => "Dunkel",
        "light" => "Hell",
        "system" => "Systemstandard",
        "about" => "Über",
        "diagnostics" => "Diagnose exportieren",
        "refresh" => "Modemdaten aktualisieren",
        "save" => "Speichern",
        "cancel" => "Abbrechen",
        _ => en(key),
    }
}

fn fr(key: &str) -> &str {
    match key {
        "preferences" => "Préférences",
        "supported_modems" => "Modems pris en charge",
        "check_updates" => "Vérifier les mises à jour",
        "release_notes" => "Notes de version",
        "report_problem" => "Signaler un problème",
        "language" => "Langue",
        "theme" => "Thème",
        "dark" => "Sombre",
        "light" => "Clair",
        "system" => "Par défaut du système",
        "about" => "À propos",
        "diagnostics" => "Exporter le diagnostic",
        "refresh" => "Actualiser le modem",
        "save" => "Enregistrer",
        "cancel" => "Annuler",
        _ => en(key),
    }
}

fn es(key: &str) -> &str {
    match key {
        "preferences" => "Preferencias",
        "supported_modems" => "Módems compatibles",
        "check_updates" => "Buscar actualizaciones",
        "release_notes" => "Notas de la versión",
        "report_problem" => "Reportar un problema",
        "language" => "Idioma",
        "theme" => "Tema",
        "dark" => "Oscuro",
        "light" => "Claro",
        "system" => "Predeterminado del sistema",
        "about" => "Acerca de",
        "diagnostics" => "Exportar diagnóstico",
        "refresh" => "Actualizar datos del módem",
        "save" => "Guardar",
        "cancel" => "Cancelar",
        _ => en(key),
    }
}

fn ru(key: &str) -> &str {
    match key {
        "preferences" => "Настройки",
        "supported_modems" => "Поддерживаемые модемы",
        "check_updates" => "Проверить обновления",
        "release_notes" => "Примечания к выпуску",
        "report_problem" => "Сообщить о проблеме",
        "language" => "Язык",
        "theme" => "Тема",
        "dark" => "Тёмная",
        "light" => "Светлая",
        "system" => "Системная",
        "about" => "О программе",
        "diagnostics" => "Экспорт диагностики",
        "refresh" => "Обновить данные модема",
        "save" => "Сохранить",
        "cancel" => "Отмена",
        _ => en(key),
    }
}

pub fn translate_ui_text(text: &str) -> String {
    let lang = language();
    if lang == Language::En {
        return text.to_string();
    }
    if lang != Language::Fa {
        return translate_other_languages(lang, text);
    }

    let exact = match text {
        // Navigation sections
        "Overview" => "وضعیت کلی",
        "Network" => "شبکه و باندها",
        "Operators" => "اپراتورها",
        "Analyzer" => "تست و تحلیل",
        "SIM & APN" => "سیم‌کارت و APN",
        "Hotspot" => "هات‌اسپات",
        "Messages" => "پیامک‌ها",
        "Calls & USSD" => "تماس و کد دستوری",
        "History" => "تاریخچه",
        "GPS & Cells" => "موقعیت و دکل‌ها",
        "Diagnostics" => "عیب‌یابی",
        "Hardware" => "سخت‌افزار",
        "Advanced" => "تنظیمات پیشرفته",
        "Pro Tools" => "ابزارهای حرفه‌ای",

        // Common UI & Overview
        "Refresh modem data" => "بروزرسانی اطلاعات مودم",
        "Preferences" => "تنظیمات",
        "Export privacy-safe diagnostics" => "خروجی گزارش بدون اطلاعات شخصی",
        "About ModemDeck by MilMit" => "درباره ModemDeck (ساخت MilMit)",
        "Live Signal Telemetry (24h)" => "نمودار زنده سیگنال (۲۴ ساعت اخیر)",
        "Radio quality" => "کیفیت آنتن و سیگنال",
        "Serving radio snapshot" => "وضعیت لحظه‌ای شبکه",
        "Serving radio" => "شبکه متصل",
        "Signal history" => "تاریخچه سیگنال",
        "Technical details" => "جزئیات فنی",
        "Overall" => "وضعیت کلی",
        "Healthy" => "سالم",
        "Warning" => "هشدار",
        "Error" => "خطا",
        "ModemManager Service" => "سرویس ModemManager",
        "ModemManager daemon is active." => "سرویس ModemManager فعال است.",
        "NetworkManager Service" => "سرویس NetworkManager",
        "NetworkManager is active for APN/routing management." => {
            "سرویس NetworkManager برای مدیریت APN و مسیر شبکه فعال است."
        }
        "WWAN RFKill / Airplane Mode" => "وضعیت کلید رادیویی / حالت هواپیما",
        "WWAN radio is unblocked." => "رادیوی مودم باز و آماده استفاده است.",
        "DNS Resolver Configuration" => "تنظیمات DNS سیستم",
        "At least one nameserver is configured in /etc/resolv.conf." => {
            "حداقل یک سرور DNS در /etc/resolv.conf تنظیم شده است."
        }
        "Default Gateway & Routing" => "دروازه پیش‌فرض و مسیریابی",
        "No default IPv4 route is currently present." => "مسیر پیش‌فرض IPv4 در حال حاضر فعال نیست.",
        "Conflicting Telephony Services" => "سرویس‌های مداخله‌گر تلفنی",
        "No conflicting telephony daemon (ofono) detected." => "سرویس متداخلی (ofono) یافت نشد.",
        "Suspend / Resume Sync Readiness" => "آمادگی همگام‌سازی Sleep / Resume",
        "systemd-logind PrepareForSleep integration is available." => {
            "یکپارچه‌سازی systemd-logind برای حالت خواب فعال است."
        }
        "Strength (RSRP)" => "قدرت آنتن (RSRP)",
        "Quality (RSRQ)" => "کیفیت سیگنال (RSRQ)",
        "Interference" => "نویز و تداخل",
        "Excellent" => "عالی",
        "Very Good" => "خیلی خوب",
        "Good" => "خوب",
        "Fair" => "متوسط",
        "Poor" => "ضعیف",
        "Very Poor" => "خیلی ضعیف",
        "Unknown" => "نامشخص",
        "Not active" => "غیرفعال",
        "Active • 4G+" => "فعال • 4G+",
        "No operator" => "بدون اپراتور",
        "Connected" => "متصل",
        "Registered" => "ثبت‌شده روی شبکه",
        "Enabled" => "روشن",
        "Disabled" => "خاموش",

        // Network & Band Manager
        "Band manager" => "انتخاب باندهای فرکانسی",
        "Band profiles" => "پروفایل‌های آماده باند",
        "Persistent Band Lock" => "قفل دائمی باند",
        "Configured / allowed bands" => "باندهای مجاز فعلی",
        "Auto" => "خودکار (Auto)",
        "Apply custom selection" => "اعمال باندهای انتخابی",
        "Keep this selection" => "تأیید و نگه داشتن این حالت",
        "Restore previous now" => "برگشت به حالت قبلی",
        "Save Lock Policy" => "ذخیره روش قفل",
        "Unlock / Auto" => "باز کردن قفل / خودکار",
        "Apply profile" => "اعمال پروفایل",
        "Save current selection as profile" => "ذخیره همین انتخاب به‌عنوان پروفایل",
        "Delete profile" => "حذف پروفایل",
        "Open Band Analyzer →" => "رفتن به تست باندها ←",
        "LTE cell lock lab" => "قفل روی یک دکل خاص (آزمایشی)",
        "Generate preview" => "ساخت پیش‌نمایش",
        "Serving cell" => "دکل متصل",
        "Scan serving cell" => "بررسی دکل متصل",
        "Carrier aggregation" => "تجمیع باند (4G+)",
        "Serving band" => "باند متصل",
        "Bandwidth" => "پهنای باند",
        "Firmware" => "نسخه فریمور",
        "Capabilities" => "قابلیت‌ها",
        "Supported bands" => "باندهای پشتیبانی‌شده",
        "Read radio details" => "خواندن جزئیات مودم",
        "Refresh radio details" => "بروزرسانی جزئیات مودم",

        // Operators, Doctor, Hardware, GPS, Pro Tools (simple everyday Persian)
        "Operator Information Center" => "اطلاعات اپراتور و سیم‌کارت",
        "Operator Database & Manual Override" => "بانک اپراتورها و انتخاب دستی",
        "Database Coverage" => "پوشش بانک اطلاعاتی",
        "Identification Confidence" => "دقت تشخیص اپراتور",
        "Roaming Status" => "وضعیت رومینگ",
        "Home Network" => "شبکه اصلی (غیررومینگ)",
        "Roaming" => "در حال رومینگ",
        "SIM Operator" => "اپراتور سیم‌کارت",
        "Country" => "کشور",
        "MCC / MNC" => "کد شبکه (MCC/MNC)",
        "MVNO / Host" => "نوع اپراتور / شبکه میزبان",
        "Known DB Bands" => "باندهای شناخته‌شده اپراتور",
        "Data Source" => "منبع اطلاعات",
        "Create Recommended APN (with backup)" => "ساخت APN پیشنهادی (با بکاپ قبلی)",
        "Backup APNs" => "بکاپ گرفتن از APNها",
        "Rollback Operator DB" => "برگرداندن بانک قبلی اپراتورها",
        "Export Operator DB" => "خروجی گرفتن از بانک اپراتورها",
        "Verified USSD & Portal Links" => "کدهای دستوری و لینک‌های رسمی",

        "GPS / GNSS Manager (Off by default)" => "موقعیت‌یاب GPS (پیش‌فرض خاموش برای حفظ باتری)",
        "Enable GPS" => "روشن کردن GPS",
        "Disable GPS" => "خاموش کردن GPS",
        "Read Fix" => "گرفتن موقعیت فعلی",
        "Export GPX" => "خروجی GPX",
        "Export CSV" => "خروجی CSV",
        "Cell & Tower Analyzer" => "اطلاعات دکل و سلول",

        "Modem Doctor & Health Diagnostics" => "عیب‌یاب خودکار مودم و اینترنت",
        "Run Modem Doctor" => "شروع عیب‌یابی",
        "Fix Safe Issues" => "رفع خودکار خطاهای ایمن",
        "FCC Lock Management" => "مدیریت قفل کارخانه‌ای (FCC Lock)",
        "Internet Diagnostics (Domestic vs International)" => {
            "تست اینترنت (مقایسه سایت‌های داخلی و خارجی)"
        }
        "Run Connectivity Probes" => "شروع تست اتصال",
        "RF Constellation & Cell Anomaly Guard" => "نمودار کیفیت سیگنال و هشدار تغییر دکل",
        "Export Shareable HTML Health Card" => "خروجی صفحه وب گزارش سلامت (HTML)",

        "Hardware Detection & Compatibility" => "شناسایی سخت‌افزار و سازگاری",
        "Host Laptop (DMI)" => "مدل لپ‌تاپ",
        "Hardware DB Models" => "تعداد مدل‌های بانک سخت‌افزار",
        "Kernel Driver" => "درایور کرنل لینوکس",
        "USB / PCI ID" => "شناسه سخت‌افزاری (USB / PCI)",
        "Commercial Name" => "نام تجاری مودم",
        "Chipset" => "تراشه (چیپست)",
        "Verification Status" => "وضعیت تست و تأیید",
        "Firmware, Bus Link & Thermal Guard" => "فریمور، سرعت درگاه و وضعیت دما",
        "eSIM / eUICC Manager" => "مدیریت سیم‌کارت الکترونیکی (eSIM)",
        "List eSIM Profiles" => "نمایش پروفایل‌های eSIM",
        "Download Profile" => "دانلود پروفایل جدید",
        "Slot 1" => "اسلات ۱",
        "Slot 2" => "اسلات ۲",
        "Backup & Privacy-Safe Hardware Compatibility Report" => {
            "بکاپ تنظیمات و گزارش سخت‌افزار (بدون اطلاعات شخصی)"
        }
        "Export Settings Backup" => "گرفتن بکاپ از تنظیمات",
        "Export Hardware Report" => "خروجی گزارش سخت‌افزار",

        "Network Mode Manager" => "انتخاب نسل شبکه (2G / 3G / 4G / 5G)",
        "3GPP Network Scanner & Operator Selection" => "جستجوی شبکه‌های موبایل اطراف",
        "Scan Visible Networks" => "جستجوی شبکه‌ها",
        "Return to Automatic" => "برگشت به انتخاب خودکار شبکه",
        "Select Manually" => "انتخاب دستی شبکه",
        "Antenna & Positioning Assistant" => "کمک‌یار پیدا کردن بهترین جای آنتن",
        "Record Position" => "ثبت این مکان",
        "RF Score" => "امتیاز کلی سیگنال",
        "Signal %" => "درصد آنتن",
        "Network Failover & Active App Sockets" => "اینترنت پشتیبان و برنامه‌های متصل",
        "Sample Active App Sockets" => "بررسی برنامه‌های در حال استفاده از اینترنت",
        "Safe AT Command Terminal" => "ترمینال امن دستورات AT",
        "Run AT Command" => "اجرای دستور",
        "Allow documented state-changing AT commands" => {
            "اجازه اجرای دستورات تغییردهنده (با احتیاط)"
        }

        // Preferences & Actions
        "Save and close" => "ذخیره و بستن",
        "Background monitor" => "مانیتور خودکار در پس‌زمینه",
        "Notifications" => "نمایش اعلان‌ها",
        "Signal refresh" => "سرعت بروزرسانی سیگنال",
        "Signal refresh (s)" => "بازه بروزرسانی سیگنال (ثانیه)",
        "Local Remote Server (127.0.0.1)" => "سرور محلی مرورگر (127.0.0.1)",
        "Start Band Scan" => "شروع تست باندها",
        "Cancel Scan" => "توقف تست",
        "Run Speed Test" => "شروع تست سرعت",
        "Enable Hotspot" => "روشن کردن هات‌اسپات",
        "Disable Hotspot" => "خاموش کردن هات‌اسپات",
        "Clear History" => "پاک کردن تاریخچه",
        "Export JSON" => "خروجی JSON",
        "Refresh" => "بروزرسانی",
        "Apply" => "اعمال",

        // Overview, Network, Analyzer, SIM, Hotspot, Messages, Calls, History, Advanced
        "Connection & Lock Summary" => "خلاصه وضعیت اتصال و قفل باند",
        "Operator" => "اپراتور",
        "Modem State" => "وضعیت مودم",
        "SIM Slot" => "اسلات سیم‌کارت",
        "Active Lock Mode" => "حالت قفل فعال",
        "Channel" => "کانال فرکانسی",
        "Band" => "باند",
        "CA topology" => "چیدمان تجمیع باند",
        "Component bandwidth" => "پهنای باند مجموع",
        "Extended signal metrics are not currently reported by this modem/driver." => {
            "جزئیات پیشرفته سیگنال (RSRP/SINR) فعلاً توسط این مودم گزارش نمی‌شود."
        }
        "A quality assessment needs RSRP, RSRQ or SNR from the modem. The raw ModemManager percentage above is still shown for reference." => {
            "برای سنجش دقیق کیفیت، مقادیر RSRP/RSRQ/SNR از مودم لازم است؛ درصد کلی سیگنال برای مرجع نمایش داده می‌شود."
        }
        "Profile changes use the 45-second safe rollback. The active profile is NOT saved until you confirm it." => {
            "تغییر پروفایل با بازگشت خودکار ۴۵ ثانیه‌ای محافظت می‌شود و تا تأیید نکنید دائمی نمی‌شود."
        }
        "This modem does not expose individual bands through ModemManager." => {
            "این مودم لیست باندهای جداگانه را در ModemManager نمایش نمی‌دهد."
        }
        "Band Analyzer — Automated Frequency Test Engine" => {
            "آزمایش خودکار و مقایسه باندهای فرکانسی"
        }
        "Best Overall" => "بهترین حالت کلی",
        "Fastest DL" => "بیشترین سرعت دانلود",
        "Lowest Latency" => "کمترین پینگ (تأخیر)",
        "Most Stable" => "پایدارترین اتصال",
        "Best Signal" => "قوی‌ترین آنتن",
        "Best Gaming" => "بهترین برای بازی آنلاین",
        "Best Streaming" => "بهترین برای استریم و ویدیو",
        "Best Work" => "بهترین برای کار و تماس",
        "Save Profile" => "ذخیره پروفایل",
        "Internet Speed Test (Modem-Interface Bound)" => "تست سرعت اینترنت (روی شبکه مودم)",
        "Allow internet speed test traffic (unchecked = Radio-only / no heavy traffic)" => {
            "اجازه مصرف اینترنت برای تست سرعت (بدون تیک = فقط تست آنتن بدون مصرف حجم)"
        }
        "Estimated data usage: ~0 MB (Radio-only mode)" => "مصرف اینترنت تخمینی: حدود ۰ مگابایت",
        "Check Internet Health Now" => "بررسی سلامت اینترنت همین الان",
        "Operator Comparison (Saved Across SIM Swaps)" => {
            "مقایسه اپراتورها (ذخیره‌شده با تعویض سیم‌کارت)"
        }
        "CA Mode" => "حالت 4G+",
        "Total Verified BW" => "کل پهنای باند",
        "Autoconnect" => "اتصال خودکار",
        "SIM lock status" => "وضعیت قفل سیم‌کارت",
        "SIM security & Multi-SIM" => "امنیت سیم‌کارت و چندسیم‌کارته",
        "SIM slot" => "اسلات سیم‌کارت",
        "Enable PIN protection" => "فعال کردن پین سیم‌کارت",
        "Disable PIN protection" => "غیرفعال کردن پین سیم‌کارت",
        "Save changes" => "ذخیره تغییرات",
        "Mobile Hotspot (Wi-Fi Connection Sharing)" => {
            "هات‌اسپات موبایل (اشتراک‌گذاری اینترنت با وای‌فای)"
        }
        "Refresh Status" => "بروزرسانی وضعیت",
        "Use WPA3 (SAE) if supported" => "استفاده از رمزنگاری WPA3 در صورت پشتیبانی",
        "State" => "وضعیت",
        "Connected Clients" => "دستگاه‌های متصل",
        "Data Shared" => "حجم اشتراک‌گذاری‌شده",
        "SMS messages" => "پیامک‌ها",
        "Load / refresh messages" => "دریافت / بروزرسانی پیامک‌ها",
        "Send SMS" => "ارسال پیامک",
        "No SMS messages are currently exposed by the modem." => "پیامکی روی مودم یافت نشد.",
        "Voice Calls" => "تماس صوتی",
        "Dial" => "شماره‌گیری",
        "Answer" => "پاسخ دادن",
        "Hangup" => "قطع تماس",
        "Refresh calls" => "بروزرسانی تماس‌ها",
        "No active calls." => "تماس فعالی وجود ندارد.",
        "Send USSD" => "ارسال کد دستوری",
        "Advanced Signal & Performance History" => "تاریخچه سیگنال و عملکرد",
        "Serving cell (ModemManager GetCellInfo)" => "اطلاعات دکل متصل",
        "Primary port" => "پورت اصلی",
        "ModemManager plugin" => "افزونه ModemManager",
        "D-Bus path" => "مسیر سیستمی D-Bus",
        "Click 'Run Modem Doctor' to inspect ModemManager, NetworkManager, RFKill, SIM, FCC Lock, Gateway, MTU, DNS, and Conflicting Services." => {
            "برای بررسی کامل سرویس‌ها، سیم‌کارت، قفل FCC، مسیر شبکه و DNS روی «شروع عیب‌یابی» کلیک کنید."
        }
        "Tests DNS, TCP/TLS reachability across domestic and international endpoints." => {
            "تست دسترسی DNS و اتصال به سرورهای داخلی و بین‌المللی."
        }
        "No scan performed yet." => "هنوز اسکن انجام نشده است.",
        "Profiles not queried yet." => "پروفایل‌های eSIM هنوز خوانده نشده‌اند.",
        "Record 2+ positions to compare signal quality." => {
            "حداقل ۲ مکان را ثبت کنید تا بهترین جا را مقایسه کند."
        }
        "Click 'Sample Active App Sockets' to view socket counts per process." => {
            "برای دیدن برنامه‌هایی که به شبکه متصل هستند کلیک کنید."
        }
        "Read-only mode active; dangerous NV/firmware commands are always blocked." => {
            "حالت فقط‌خواندنی فعال است؛ دستورات خطرناک فریمور همیشه مسدود هستند."
        }
        "Data usage & live traffic" => "مصرف اینترنت و سرعت لحظه‌ای",
        "Download" => "دانلود",
        "Upload" => "آپلود",
        "Today" => "امروز",
        "This month" => "این ماه",
        "Reset usage counters" => "صفر کردن آمار مصرف",
        "Confidence" => "دقت ارزیابی",
        "High" => "بالا",
        "Medium" => "متوسط",
        "Low" => "کم",
        "Automatic" => "خودکار",
        "2G Only" => "فقط 2G",
        "3G Only" => "فقط 3G",
        "4G LTE Only" => "فقط 4G LTE",
        "5G NR Only" => "فقط 5G NR",
        "4G Preferred" => "اولویت با 4G",
        "5G Preferred" => "اولویت با 5G",
        "4G + 5G" => "4G و 5G",
        "LTE FDD" => "باندهای LTE FDD",
        "TD-LTE" => "باندهای TD-LTE",
        "Exact" => "دقیق",
        "Primary MNO" => "اپراتور اصلی",
        "Carrier Aggregation Analyzer" => "تحلیلگر تجمیع باند (4G+)",
        "Single Carrier (CA Inactive)" => "تک‌باند (4G+ غیرفعال)",
        "Connection Stability Monitor" => "پایش پایداری اتصال",
        "No GNSS fix yet (ensure GPS is enabled and antenna has sky view)." => {
            "هنوز موقعیت GPS دریافت نشده (از روشن بودن GPS و دید آسمان مطمئن شوید)."
        }
        "Serving Cells" => "دکل متصل",
        "No serving cell details cached yet (use Network -> Scan serving cell)." => {
            "اطلاعات دکل هنوز ثبت نشده (از بخش تنظیمات پیشرفته اسکن کنید)."
        }
        "FCC Lock Status" => "وضعیت قفل کارخانه‌ای",
        "Unlocked" => "باز (Unlocked)",
        "Official Unlock Script" => "اسکریپت رسمی بازگشایی",
        "Not enabled in /etc/ModemManager/fcc-unlock.d" => {
            "در /etc/ModemManager/fcc-unlock.d فعال نیست"
        }
        "Current Firmware" => "نسخه فریمور فعلی",
        "fwupd Available" => "سرویس fwupd در دسترس است",
        "Yes" => "بله",
        "No" => "خیر",
        "LVFS Updates" => "آپدیت‌های رسمی LVFS",
        "None reported" => "آپدیتی گزارش نشده",
        "Bus Link & PM" => "سرعت درگاه و مدیریت توان",
        "Thermal Guard" => "پایش دما",
        "CA Efficiency" => "بهره‌وری تجمیع باند",
        "Hardware Verified" => "تأییدشده با سخت‌افزار واقعی",
        "Experimental" => "آزمایشی",
        "Documented" => "مستندسازی‌شده",
        "Band Lock Status" => "وضعیت قفل باند",
        "Cell Lock Status" => "وضعیت قفل سلول",
        "eSIM Status" => "وضعیت eSIM",
        "lpac Installed" => "ابزار lpac نصب است",
        "Status" => "وضعیت",
        "Capabilities are enabled strictly from runtime detection and verified driver profiles. Unsupported or unverified write commands remain disabled." => {
            "قابلیت‌ها فقط بر اساس تشخیص واقعی سخت‌افزار فعال می‌شوند و دستورات تاییدنشده غیرفعال می‌مانند."
        }
        "Configured bands are allowed bands, not necessarily the currently serving LTE/5G carrier." => {
            "باندهای مجاز الزاماً به معنی باند فعال لحظه‌ای نیستند."
        }
        "Scanning for visible cellular networks may temporarily interrupt an active mobile broadband data session." => {
            "جستجوی شبکه‌های اطراف ممکن است اتصال اینترنت موبایل را چند لحظه قطع کند."
        }
        "MCCMNC (e.g. 43235)" => "کد شبکه (مثلاً 43235)",
        "Position label (e.g. Window, Desk)" => "نام مکان (مثلاً پنجره، میز)",
        "Per-Antenna Chains" => "وضعیت آنتن‌های مجزا",
        "Aggregate Only" => "فقط سیگنال تجمیعی",
        "MIMO" => "چیدمان MIMO",
        "Modulation" => "مدولاسیون",
        "Prefer Wi-Fi" => "اولویت با وای‌فای",
        "Prefer Ethernet" => "اولویت با کابل شبکه",
        "Prefer Mobile Broadband" => "اولویت با اینترنت سیم‌کارت",
        _ => "",
    };
    if !exact.is_empty() {
        return exact.to_string();
    }
    if let Some(rest) = text.strip_prefix("Modem signal  ") {
        return format!("قدرت آنتن مودم  {rest}");
    }
    if let Some(rest) = text.strip_prefix("Detected band mode: ") {
        return format!("حالت باند فعلی: {rest}");
    }
    if let Some(rest) = text.strip_suffix(" radio metrics") {
        return format!("جزئیات سیگنال {rest}");
    }
    if let Some(rest) = text.strip_prefix("Configured (allowed) bands: ") {
        return format!("باندهای مجاز فعلی: {rest}");
    }
    if let Some(rest) = text.strip_prefix("Lock status: ") {
        return format!("وضعیت قفل: {rest}");
    }
    if let Some(rest) = text.strip_prefix("Enable Official FCC-Unlock ") {
        return format!("فعال‌سازی بازگشایی رسمی FCC {rest}");
    }
    if let Some(rest) = text.strip_prefix("Auto-Band Autopilot: ") {
        return format!("پیشنهاد خودکار باند: {rest}");
    }
    if let Some(rest) = text.strip_prefix("Fair (") {
        return format!("متوسط ({rest}");
    }
    if let Some(rest) = text.strip_prefix("Good (") {
        return format!("خوب ({rest}");
    }
    if let Some(rest) = text.strip_prefix("Very Good (") {
        return format!("خیلی خوب ({rest}");
    }
    if let Some(rest) = text.strip_prefix("Excellent (") {
        return format!("عالی ({rest}");
    }
    if let Some(rest) = text.strip_prefix("Poor (") {
        return format!("ضعیف ({rest}");
    }
    if let Some(rest) = text.strip_prefix("[Normal] ") {
        return format!("[عادی] {rest}");
    }
    if let Some(rest) = text.strip_prefix("Vendor telemetry • ") {
        return format!("اطلاعات تخصصی مودم • {rest}");
    }
    text.to_string()
}

fn translate_other_languages(lang: Language, text: &str) -> String {
    match (lang, text) {
        // Sidebar sections for AR, TR, DE, FR, ES, RU
        (Language::Ar, "Overview") => "نظرة عامة",
        (Language::Ar, "Network") => "الشبكة",
        (Language::Ar, "Operators") => "المشغلون",
        (Language::Ar, "Analyzer") => "المحلل",
        (Language::Ar, "SIM & APN") => "الشريحة و APN",
        (Language::Ar, "Hotspot") => "نقطة اتصال",
        (Language::Ar, "Messages") => "الرسائل",
        (Language::Ar, "Calls & USSD") => "المكالمات و USSD",
        (Language::Ar, "History") => "السجل",
        (Language::Ar, "GPS & Cells") => "الموقع والأبراج",
        (Language::Ar, "Diagnostics") => "التشخيص",
        (Language::Ar, "Hardware") => "العتاد",
        (Language::Ar, "Advanced") => "متقدم",
        (Language::Ar, "Pro Tools") => "أدوات احترافية",
        (Language::Ar, "Save and close") => "حفظ وإغلاق",

        (Language::Tr, "Overview") => "Genel Bakış",
        (Language::Tr, "Network") => "Ağ ve Bantlar",
        (Language::Tr, "Operators") => "Operatörler",
        (Language::Tr, "Analyzer") => "Analizör",
        (Language::Tr, "SIM & APN") => "SIM ve APN",
        (Language::Tr, "Hotspot") => "Erişim Noktası",
        (Language::Tr, "Messages") => "Mesajlar",
        (Language::Tr, "Calls & USSD") => "Aramalar ve USSD",
        (Language::Tr, "History") => "Geçmiş",
        (Language::Tr, "GPS & Cells") => "GPS ve Baz İstasyonları",
        (Language::Tr, "Diagnostics") => "Tanılama",
        (Language::Tr, "Hardware") => "Donanım",
        (Language::Tr, "Advanced") => "Gelişmiş",
        (Language::Tr, "Pro Tools") => "Uzman Araçları",
        (Language::Tr, "Save and close") => "Kaydet ve kapat",

        (Language::De, "Overview") => "Übersicht",
        (Language::De, "Network") => "Netz & Bänder",
        (Language::De, "Operators") => "Anbieter",
        (Language::De, "Analyzer") => "Analysator",
        (Language::De, "SIM & APN") => "SIM & APN",
        (Language::De, "Hotspot") => "Hotspot",
        (Language::De, "Messages") => "Nachrichten",
        (Language::De, "Calls & USSD") => "Anrufe & USSD",
        (Language::De, "History") => "Verlauf",
        (Language::De, "GPS & Cells") => "GPS & Funkzellen",
        (Language::De, "Diagnostics") => "Diagnose",
        (Language::De, "Hardware") => "Hardware",
        (Language::De, "Advanced") => "Erweitert",
        (Language::De, "Pro Tools") => "Profi-Tools",
        (Language::De, "Save and close") => "Speichern und schließen",

        (Language::Fr, "Overview") => "Aperçu",
        (Language::Fr, "Network") => "Réseau & Bandes",
        (Language::Fr, "Operators") => "Opérateurs",
        (Language::Fr, "Analyzer") => "Analyseur",
        (Language::Fr, "SIM & APN") => "SIM & APN",
        (Language::Fr, "Hotspot") => "Point d'accès",
        (Language::Fr, "Messages") => "Messages",
        (Language::Fr, "Calls & USSD") => "Appels & USSD",
        (Language::Fr, "History") => "Historique",
        (Language::Fr, "GPS & Cells") => "GPS & Cellules",
        (Language::Fr, "Diagnostics") => "Diagnostics",
        (Language::Fr, "Hardware") => "Matériel",
        (Language::Fr, "Advanced") => "Avancé",
        (Language::Fr, "Pro Tools") => "Outils Pro",
        (Language::Fr, "Save and close") => "Enregistrer et fermer",

        (Language::Es, "Overview") => "Resumen",
        (Language::Es, "Network") => "Red y Bandas",
        (Language::Es, "Operators") => "Operadores",
        (Language::Es, "Analyzer") => "Analizador",
        (Language::Es, "SIM & APN") => "SIM y APN",
        (Language::Es, "Hotspot") => "Zona Wi-Fi",
        (Language::Es, "Messages") => "Mensajes",
        (Language::Es, "Calls & USSD") => "Llamadas y USSD",
        (Language::Es, "History") => "Historial",
        (Language::Es, "GPS & Cells") => "GPS y Celdas",
        (Language::Es, "Diagnostics") => "Diagnóstico",
        (Language::Es, "Hardware") => "Hardware",
        (Language::Es, "Advanced") => "Avanzado",
        (Language::Es, "Pro Tools") => "Herramientas Pro",
        (Language::Es, "Save and close") => "Guardar y cerrar",

        (Language::Ru, "Overview") => "Обзор",
        (Language::Ru, "Network") => "Сеть и диапазоны",
        (Language::Ru, "Operators") => "Операторы",
        (Language::Ru, "Analyzer") => "Анализатор",
        (Language::Ru, "SIM & APN") => "SIM и APN",
        (Language::Ru, "Hotspot") => "Точка доступа",
        (Language::Ru, "Messages") => "Сообщения",
        (Language::Ru, "Calls & USSD") => "Звонки и USSD",
        (Language::Ru, "History") => "История",
        (Language::Ru, "GPS & Cells") => "GPS и вышки",
        (Language::Ru, "Diagnostics") => "Диагностика",
        (Language::Ru, "Hardware") => "Оборудование",
        (Language::Ru, "Advanced") => "Дополнительно",
        (Language::Ru, "Pro Tools") => "Про-инструменты",
        (Language::Ru, "Save and close") => "Сохранить и закрыть",
        _ => return text.to_string(),
    }
    .to_string()
}
