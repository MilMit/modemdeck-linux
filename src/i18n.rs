use std::sync::{OnceLock, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language { En, Fa }
static LANG: OnceLock<RwLock<Language>> = OnceLock::new();

pub fn init(value: &str) {
    let lang = match value {
        "fa" => Language::Fa,
        "en" => Language::En,
        _ => if std::env::var("LANG").unwrap_or_default().to_ascii_lowercase().starts_with("fa") { Language::Fa } else { Language::En },
    };
    let lock = LANG.get_or_init(|| RwLock::new(lang));
    if let Ok(mut current) = lock.write() { *current = lang; }
}

pub fn language() -> Language { LANG.get_or_init(|| RwLock::new(Language::En)).read().map(|v| *v).unwrap_or(Language::En) }
pub fn tr<'a>(key: &'a str) -> &'a str { if language() == Language::Fa { fa(key) } else { en(key) } }

fn en<'a>(key: &'a str) -> &'a str { match key {
    "preferences" => "Preferences", "supported_modems" => "Supported modems", "check_updates" => "Check for updates",
    "release_notes" => "Release notes", "report_problem" => "Report a problem", "apn_manager" => "APN manager",
    "sim_security" => "SIM security", "unlock_sim" => "Unlock SIM", "data_usage" => "Data usage & live traffic",
    "profiles" => "Band profiles", "restore_profile" => "Restore profile after reboot/resume", "save_profile" => "Save current selection as profile",
    "apply_profile" => "Apply profile", "delete_profile" => "Delete profile", "language" => "Language", "system" => "System default",
    "english" => "English", "persian" => "فارسی", "today" => "Today", "this_month" => "This month", "download" => "Download",
    "upload" => "Upload", "live_speed" => "Live speed", "reset_usage" => "Reset usage counters", "create_apn" => "Create APN profile",
    "activate" => "Activate", "delete" => "Delete", "edit" => "Edit", "save" => "Save", "cancel" => "Cancel",
    "about" => "About", "diagnostics" => "Export diagnostics", "refresh" => "Refresh modem data", "hotplug" => "Hot-plug ready",
    "suspend_resume" => "Suspend/resume aware", "crash_safe" => "Crash-safe rollback armed", "website" => "MilMit website",
    _ => key,
}}

fn fa<'a>(key: &'a str) -> &'a str { match key {
    "preferences" => "تنظیمات", "supported_modems" => "مودم‌های پشتیبانی‌شده", "check_updates" => "بررسی بروزرسانی",
    "release_notes" => "یادداشت‌های انتشار", "report_problem" => "گزارش مشکل", "apn_manager" => "مدیریت APN",
    "sim_security" => "امنیت سیم‌کارت", "unlock_sim" => "باز کردن قفل سیم‌کارت", "data_usage" => "مصرف داده و سرعت لحظه‌ای",
    "profiles" => "پروفایل‌های باند", "restore_profile" => "بازیابی پروفایل پس از راه‌اندازی/Resume", "save_profile" => "ذخیره انتخاب فعلی به‌عنوان پروفایل",
    "apply_profile" => "اعمال پروفایل", "delete_profile" => "حذف پروفایل", "language" => "زبان", "system" => "پیش‌فرض سیستم",
    "english" => "English", "persian" => "فارسی", "today" => "امروز", "this_month" => "این ماه", "download" => "دانلود",
    "upload" => "آپلود", "live_speed" => "سرعت لحظه‌ای", "reset_usage" => "صفر کردن آمار مصرف", "create_apn" => "ساخت پروفایل APN",
    "activate" => "فعال‌سازی", "delete" => "حذف", "edit" => "ویرایش", "save" => "ذخیره", "cancel" => "انصراف",
    "about" => "درباره", "diagnostics" => "خروجی عیب‌یابی", "refresh" => "بروزرسانی اطلاعات مودم", "hotplug" => "تشخیص اتصال/جداسازی",
    "suspend_resume" => "سازگار با Sleep/Resume", "crash_safe" => "Rollback ایمن در برابر کرش", "website" => "وب‌سایت MilMit",
    _ => en(key),
}}

pub fn translate_ui_text(text: &str) -> String {
    if language() != Language::Fa { return text.to_string(); }
    let exact = match text {
        "Refresh modem data" => "بروزرسانی اطلاعات مودم",
        "Preferences" => "تنظیمات",
        "Export privacy-safe diagnostics" => "خروجی عیب‌یابی بدون اطلاعات حساس",
        "About ModemDeck by MilMit" => "درباره ModemDeck از MilMit",
        "Radio quality" => "کیفیت رادیویی",
        "Serving radio snapshot" => "وضعیت شبکه متصل",
        "Serving radio" => "رادیوی متصل",
        "Signal history" => "تاریخچه سیگنال",
        "Band manager" => "مدیریت باند",
        "LTE cell lock lab" => "آزمایشگاه قفل سلول LTE",
        "Technical details" => "جزئیات فنی",
        "Configured / allowed bands" => "باندهای مجاز / تنظیم‌شده",
        "Auto" => "خودکار",
        "LTE FDD" => "LTE FDD",
        "TD-LTE" => "TD-LTE",
        "Apply custom selection" => "اعمال انتخاب سفارشی",
        "Keep this selection" => "تأیید این انتخاب",
        "Restore previous now" => "بازگردانی تنظیم قبلی",
        "Generate preview" => "ساخت پیش‌نمایش",
        "Serving cell" => "سلول متصل",
        "Scan serving cell" => "اسکن سلول متصل",
        "Carrier aggregation" => "تجمیع حامل",
        "Serving band" => "باند متصل",
        "Bandwidth" => "پهنای باند",
        "Duplex" => "نوع Duplex",
        "Firmware" => "فریمور",
        "Capabilities" => "قابلیت‌ها",
        "Supported bands" => "باندهای پشتیبانی‌شده",
        "ModemManager plugin" => "پلاگین ModemManager",
        "Primary port" => "پورت اصلی",
        "D-Bus path" => "مسیر D-Bus",
        "Overall" => "وضعیت کلی",
        "Strength (RSRP)" => "قدرت (RSRP)",
        "Quality (RSRQ)" => "کیفیت (RSRQ)",
        "Interference" => "تداخل",
        "Excellent" => "عالی",
        "Good" => "خوب",
        "Fair" => "متوسط",
        "Poor" => "ضعیف",
        "Unknown" => "نامشخص",
        "Not active" => "غیرفعال",
        "Active • 4G+" => "فعال • 4G+",
        "Read radio details" => "خواندن جزئیات رادیویی",
        "Refresh radio details" => "بروزرسانی جزئیات رادیویی",
        "Save and close" => "ذخیره و بستن",
        "Background monitor" => "مانیتور پس‌زمینه",
        "Notifications" => "اعلان‌ها",
        "Signal refresh" => "بازه بروزرسانی سیگنال",
        "Autoconnect" => "اتصال خودکار",
        "Enable PIN protection" => "فعال‌کردن محافظت PIN",
        "Disable PIN protection" => "غیرفعال‌کردن محافظت PIN",
        "Save changes" => "ذخیره تغییرات",
        "Data usage & live traffic" => "مصرف داده و سرعت لحظه‌ای",
        "SIM security" => "امنیت سیم‌کارت",
        "APN manager" => "مدیریت APN",
        "Band profiles" => "پروفایل‌های باند",
        "Reset usage counters" => "صفر کردن آمار مصرف",
        "Create APN profile" => "ساخت پروفایل APN",
        "Activate" => "فعال‌سازی",
        "Delete" => "حذف",
        "Apply profile" => "اعمال پروفایل",
        "Delete profile" => "حذف پروفایل",
        "Save current selection as profile" => "ذخیره انتخاب فعلی به‌عنوان پروفایل",
        "Connection name" => "نام اتصال",
        "Username (optional)" => "نام کاربری (اختیاری)",
        "Password (optional; leave blank to keep existing)" => "رمز عبور (اختیاری؛ برای حفظ رمز فعلی خالی بگذارید)",
        "Profile name" => "نام پروفایل",
        _ => "",
    };
    if !exact.is_empty() { return exact.to_string(); }
    if let Some(rest)=text.strip_prefix("Modem signal  ") { return format!("سیگنال مودم  {rest}"); }
    if let Some(rest)=text.strip_prefix("Detected band mode: ") { return format!("حالت باند تشخیص‌داده‌شده: {rest}"); }
    if let Some(rest)=text.strip_prefix("Read source: ") { return format!("منبع خواندن: {rest}"); }
    if let Some(rest)=text.strip_prefix("Driver profile • ") { return format!("پروفایل درایور • {rest}"); }
    if let Some(rest)=text.strip_prefix("Vendor driver • ") { return format!("درایور سازنده • {rest}"); }
    text.to_string()
}
